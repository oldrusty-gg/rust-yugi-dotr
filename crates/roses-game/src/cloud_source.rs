//! Cloud animation inputs.
//!
//! Two manifest profiles share one loader entry point:
//! - `roses-private-c400-checkpoint-1`: a captured c400 state plus an exported
//!   sine table (research resume; the RNG is a captured state).
//! - `roses-disc-boot-c400-1`: everything derived from the user's ISO at run
//!   time. The quarter-wave sine table is read from the boot executable through
//!   its ELF program headers, the atlas from `DATA/TEXEVE.MRG`, the RNG from
//!   the C library boot seed advanced by the main-loop counters of an explicit
//!   [`BootSchedule`], and the counter-0 state from the original initialisation
//!   arithmetic. No expected draw list or captured state is involved.
use crate::clouds::{BootSchedule, CloudState, SINE_TABLE_ENTRIES, SineTable, SourceRng};
use crate::ee_float::FloatModel;
use crate::{Result, bounded_read, hash_valid, validate_relative_path};
use roses_assets::{OpeningArchiveKind, OpeningPaletteFormat, decode_pal_opening_indexed_texture};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::Path;

pub const CHECKPOINT_PROFILE: &str = "roses-private-c400-checkpoint-1";
pub const DISC_BOOT_PROFILE: &str = "roses-disc-boot-c400-1";
/// EE virtual address of the 1,025-entry quarter-wave sine table read by the
/// original sine helper 0x20ACC0 (0x20AD28..0x20AD3C index adjacent entries).
pub const SINE_TABLE_ADDRESS: u32 = 0x29_3430;
const EXECUTABLE_PATH: &str = "SLES_515.79";
const MAX_EXECUTABLE_BYTES: u64 = 2_000_000;
const MAX_ARCHIVE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFile {
    pub path: String,
    pub sha256: String,
}
#[derive(Deserialize)]
struct ProfileProbe {
    schema_version: u32,
    profile: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CloudSource {
    schema_version: u32,
    profile: String,
    state: SourceFile,
    sine: SourceFile,
    texture_archive: SourceFile,
    rng_state_hex: String,
    source_state_sha256: String,
    // These are scheduler context, not RNG calls owned by c400.
    external_rng_calls_per_tick: u32,
    #[serde(default)]
    emission_checkpoint: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiscBootSource {
    schema_version: u32,
    profile: String,
    /// Root-relative path of the user's ISO; its boot executable must match
    /// the PAL fingerprint. Or, from a game import, the two files it holds
    /// (`executable_path` = SLES_515.79, `texeve_path` = DATA/TEXEVE.MRG).
    #[serde(default)]
    iso_path: Option<String>,
    #[serde(default)]
    executable_path: Option<String>,
    #[serde(default)]
    texeve_path: Option<String>,
    schedule: BootSchedule,
    arithmetic: Arithmetic,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Arithmetic {
    Pcsx2PositiveRounding,
    HostF32,
}
impl From<Arithmetic> for FloatModel {
    fn from(value: Arithmetic) -> Self {
        match value {
            Arithmetic::Pcsx2PositiveRounding => Self::Pcsx2PositiveRounding,
            Arithmetic::HostF32 => Self::HostF32,
        }
    }
}
pub struct LoadedCloudSource {
    pub state: CloudState,
    pub sine: SineTable,
    pub rng: SourceRng,
    pub rgba_gs: Vec<u8>,
    pub external_rng_calls_per_tick: u32,
    pub input_sha256: String,
    /// Checkpoint: the save state's fingerprint. Disc boot: the executable's.
    pub source_state_sha256: String,
    pub emission_checkpoint: bool,
    /// Present only for a disc-derived boot start.
    pub boot_schedule: Option<BootSchedule>,
}
fn source_file(root: &Path, source: &SourceFile, max: u64) -> Result<Vec<u8>> {
    validate_relative_path(&source.path)?;
    if !hash_valid(&source.sha256) {
        return Err("Invalid cloud input fingerprint".into());
    }
    let path = root_child(root, &source.path)?;
    let bytes = bounded_read(&path, max)?;
    if format!("{:x}", Sha256::digest(&bytes)) != source.sha256.to_ascii_lowercase() {
        return Err(format!("Cloud input fingerprint mismatch: {}", source.path));
    }
    Ok(bytes)
}
fn root_child(root: &Path, relative: &str) -> Result<std::path::PathBuf> {
    validate_relative_path(relative)?;
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| format!("{relative}: {e}"))?;
    if !path.starts_with(root) {
        return Err("Cloud source escapes data root".into());
    }
    Ok(path)
}
/// GS-order RGBA bytes of TEXEVE record 25 (the cloud atlas), original
/// indexed pixels through the raw palette words.
fn atlas_rgba_gs(archive: &[u8]) -> Result<Vec<u8>> {
    let texture = decode_pal_opening_indexed_texture(archive, OpeningArchiveKind::TexEve, 25)
        .map_err(|e| e.to_string())?;
    if (texture.width, texture.height, texture.palette_format)
        != (128, 64, OpeningPaletteFormat::Rgba8888Alpha128)
    {
        return Err("Cloud atlas does not match the source texture layout".into());
    }
    Ok(texture
        .palette_indices
        .iter()
        .flat_map(|&i| texture.palette_words_raw[i as usize].to_le_bytes())
        .collect())
}
/// Map an EE virtual address range to file bytes through the ELF32 program
/// headers; only a range inside one PT_LOAD file image is supported.
pub fn elf_load_bytes(executable: &[u8], address: u32, length: u32) -> Result<&[u8]> {
    let u16_at = |at: usize| -> Result<u16> {
        executable
            .get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .ok_or_else(|| "Executable header truncated".to_string())
    };
    let u32_at = |at: usize| -> Result<u32> {
        executable
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| "Executable header truncated".to_string())
    };
    if executable.get(0..7) != Some(&[0x7f, b'E', b'L', b'F', 1, 1, 1][..]) {
        return Err("Boot executable is not a little-endian ELF32 image".into());
    }
    let (phoff, phentsize, phnum) = (
        u32_at(0x1c)? as usize,
        u16_at(0x2a)? as usize,
        u16_at(0x2c)? as usize,
    );
    // The PAL executable carries 840 program headers (one per module image).
    if phentsize < 32 || phnum > 4096 {
        return Err("Executable program header table unsupported".into());
    }
    let end = address
        .checked_add(length)
        .ok_or_else(|| "Executable address range overflows".to_string())?;
    for index in 0..phnum {
        let base = phoff + index * phentsize;
        if u32_at(base)? != 1 {
            continue; // PT_LOAD only
        }
        let (offset, vaddr, filesz) = (u32_at(base + 4)?, u32_at(base + 8)?, u32_at(base + 16)?);
        let Some(segment_end) = vaddr.checked_add(filesz) else {
            continue;
        };
        if address >= vaddr && end <= segment_end {
            let start = offset as usize + (address - vaddr) as usize;
            return executable
                .get(start..start + length as usize)
                .ok_or_else(|| "Executable segment exceeds the file".to_string());
        }
    }
    Err(format!(
        "Address {address:#x} is not inside a loaded executable segment"
    ))
}
fn load_checkpoint(bytes: &[u8], root: &Path) -> Result<LoadedCloudSource> {
    let source: CloudSource = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if source.schema_version != 1
        || source.profile != CHECKPOINT_PROFILE
        || !hash_valid(&source.source_state_sha256)
        || source.external_rng_calls_per_tick > 16
    {
        return Err("Unsupported cloud checkpoint profile or scheduler context".into());
    }
    let state = CloudState::from_source_bytes(
        &source_file(root, &source.state, 11600)?,
        FloatModel::Pcsx2PositiveRounding,
    )?;
    let sine = SineTable::from_le_bytes(&source_file(root, &source.sine, 4100)?)?;
    let seed_text = source
        .rng_state_hex
        .strip_prefix("0x")
        .unwrap_or(&source.rng_state_hex);
    if seed_text.len() != 16 || !seed_text.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Cloud RNG state requires sixteen hex digits".into());
    }
    let seed = u64::from_str_radix(seed_text, 16).map_err(|e| e.to_string())?;
    let rgba_gs = atlas_rgba_gs(&source_file(
        root,
        &source.texture_archive,
        MAX_ARCHIVE_BYTES,
    )?)?;
    Ok(LoadedCloudSource {
        state,
        sine,
        rng: SourceRng::new(seed),
        rgba_gs,
        external_rng_calls_per_tick: source.external_rng_calls_per_tick,
        input_sha256: format!("{:x}", Sha256::digest(bytes)),
        source_state_sha256: source.source_state_sha256,
        emission_checkpoint: source.emission_checkpoint,
        boot_schedule: None,
    })
}
fn load_disc_boot(bytes: &[u8], root: &Path) -> Result<LoadedCloudSource> {
    let source: DiscBootSource = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if source.schema_version != 1 || source.profile != DISC_BOOT_PROFILE {
        return Err("Unsupported disc boot cloud profile".into());
    }
    source.schedule.rand_calls_before_init()?;
    let (executable, archive) = match (
        &source.iso_path,
        &source.executable_path,
        &source.texeve_path,
    ) {
        (Some(iso), None, None) => {
            let iso = root_child(root, iso)?;
            let mut disc = roses_disc::Disc::open(&iso).map_err(|e| e.to_string())?;
            (
                disc.read_file(EXECUTABLE_PATH, MAX_EXECUTABLE_BYTES)
                    .map_err(|e| e.to_string())?,
                disc.read_file(OpeningArchiveKind::TexEve.iso_path(), MAX_ARCHIVE_BYTES)
                    .map_err(|e| e.to_string())?,
            )
        }
        (None, Some(executable), Some(texeve)) => (
            bounded_read(&root_child(root, executable)?, MAX_EXECUTABLE_BYTES)?,
            bounded_read(&root_child(root, texeve)?, MAX_ARCHIVE_BYTES)?,
        ),
        _ => {
            return Err(
                "Disc boot cloud source needs iso_path, or executable_path and texeve_path".into(),
            );
        }
    };
    let executable_sha256 = format!("{:x}", Sha256::digest(&executable));
    if executable_sha256 != roses_disc::PAL_EXECUTABLE_SHA256 {
        return Err("Boot executable does not match the supported PAL build".into());
    }
    let sine = SineTable::from_le_bytes(elf_load_bytes(
        &executable,
        SINE_TABLE_ADDRESS,
        (SINE_TABLE_ENTRIES * 4) as u32,
    )?)?;
    if format!("{:x}", Sha256::digest(&archive)) != OpeningArchiveKind::TexEve.expected_sha256() {
        return Err("TEXEVE archive does not match the supported PAL build".into());
    }
    let rgba_gs = atlas_rgba_gs(&archive)?;
    let (state, rng) = CloudState::from_boot(&source.schedule, source.arithmetic.into())?;
    Ok(LoadedCloudSource {
        state,
        sine,
        rng,
        rgba_gs,
        external_rng_calls_per_tick: crate::clouds::EXTERNAL_RNG_CALLS_PER_TICK,
        input_sha256: format!("{:x}", Sha256::digest(bytes)),
        source_state_sha256: executable_sha256,
        emission_checkpoint: false,
        boot_schedule: Some(source.schedule),
    })
}
pub fn load_cloud_source(path: &Path, root: &Path) -> Result<LoadedCloudSource> {
    let bytes = bounded_read(path, 16384)?;
    let probe: ProfileProbe = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    match (probe.schema_version, probe.profile.as_str()) {
        (1, CHECKPOINT_PROFILE) => load_checkpoint(&bytes, &root),
        (1, DISC_BOOT_PROFILE) => load_disc_boot(&bytes, &root),
        _ => Err("Unsupported cloud checkpoint profile or scheduler context".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
    struct Fixture {
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = loop {
                let suffix = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "roses-cloud-source-{}-{suffix}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => break path.canonicalize().unwrap(),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("Cannot create test directory: {error}"),
                }
            };
            let fixture = Self { root };
            let mut state = vec![0; crate::clouds::SOURCE_STATE_BYTES];
            for group in 0..6 {
                let base = group * 1928;
                state[base + 24] = 1;
                state[base + 28..base + 32].copy_from_slice(&3_u32.to_le_bytes());
                for i in 0..128 {
                    state[base + 160 + 4 * i..base + 164 + 4 * i]
                        .copy_from_slice(&0.5_f32.to_le_bytes());
                    state[base + 672 + 2 * i] = 128;
                    state[base + 673 + 2 * i] = 64;
                }
            }
            fs::write(fixture.root.join("state.bin"), state).unwrap();
            let sine: Vec<_> = (0..=1024)
                .flat_map(|i| (i as f32 / 1024.).to_le_bytes())
                .collect();
            fs::write(fixture.root.join("sine.bin"), sine).unwrap();
            fs::write(
                fixture.root.join("texture.bin"),
                b"invented non-retail bytes",
            )
            .unwrap();
            fixture
        }
        fn source(&self, name: &str) -> Value {
            json!({"path":name,"sha256":format!("{:x}",Sha256::digest(fs::read(self.root.join(name)).unwrap()))})
        }
        fn manifest(&self) -> Value {
            json!({
                "schema_version":1,"profile":CHECKPOINT_PROFILE,
                "state":self.source("state.bin"),"sine":self.source("sine.bin"),
                "texture_archive":self.source("texture.bin"),"rng_state_hex":"0000000000000001",
                "source_state_sha256":"0".repeat(64),"external_rng_calls_per_tick":2
            })
        }
        fn boot_manifest(&self) -> Value {
            json!({
                "schema_version":1,"profile":DISC_BOOT_PROFILE,
                "iso_path":"not-a-disc.iso",
                "schedule":{"loop_ticks_before_init":1222,"flips_before_init":1223},
                "arithmetic":"pcsx2_positive_rounding"
            })
        }
        fn failure(&self, manifest: &Value) -> String {
            let path = self.root.join("source.json");
            fs::write(&path, serde_json::to_vec(manifest).unwrap()).unwrap();
            match load_cloud_source(&path, &self.root) {
                Ok(_) => panic!("Synthetic invalid input unexpectedly accepted"),
                Err(error) => error,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // Only the flat files/empty directories created by this fixture;
            // never recursively delete a computed temporary path.
            for entry in fs::read_dir(&self.root).unwrap().flatten() {
                let path = entry.path();
                if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                    let _ = fs::remove_dir(path);
                } else {
                    let _ = fs::remove_file(path);
                }
            }
            let _ = fs::remove_dir(&self.root);
        }
    }

    /// A minimal synthetic ELF32 with two PT_LOAD segments; no game bytes.
    fn synthetic_elf() -> Vec<u8> {
        let mut elf = vec![0_u8; 0x200];
        elf[0..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 1, 1, 1]);
        elf[0x1c..0x20].copy_from_slice(&0x34_u32.to_le_bytes());
        elf[0x2a..0x2c].copy_from_slice(&32_u16.to_le_bytes());
        elf[0x2c..0x2e].copy_from_slice(&2_u16.to_le_bytes());
        let mut segment = |index: usize, offset: u32, vaddr: u32, filesz: u32| {
            let base = 0x34 + index * 32;
            elf[base..base + 4].copy_from_slice(&1_u32.to_le_bytes());
            elf[base + 4..base + 8].copy_from_slice(&offset.to_le_bytes());
            elf[base + 8..base + 12].copy_from_slice(&vaddr.to_le_bytes());
            elf[base + 16..base + 20].copy_from_slice(&filesz.to_le_bytes());
        };
        segment(0, 0x80, 0x10_0000, 0x40);
        segment(1, 0x100, 0x20_0000, 0x80);
        for (i, byte) in elf[0x100..0x180].iter_mut().enumerate() {
            *byte = i as u8;
        }
        elf
    }

    #[test]
    fn elf_segments_map_addresses_and_reject_ranges_outside_loaded_images() {
        let elf = synthetic_elf();
        assert_eq!(
            elf_load_bytes(&elf, 0x20_0010, 4).unwrap(),
            &[16, 17, 18, 19]
        );
        assert_eq!(
            elf_load_bytes(&elf, 0x20_007c, 4).unwrap(),
            &[124, 125, 126, 127]
        );
        assert!(elf_load_bytes(&elf, 0x20_007d, 4).is_err());
        assert!(elf_load_bytes(&elf, 0x10_0000, 0x41).is_err());
        assert!(elf_load_bytes(&elf, 0x30_0000, 4).is_err());
        assert!(elf_load_bytes(&elf, u32::MAX, 4).is_err());
        assert!(elf_load_bytes(&elf[..0x40], 0x10_0000, 4).is_err());
        assert!(elf_load_bytes(b"MZ", 0x10_0000, 4).is_err());
        let mut truncated = elf.clone();
        truncated.truncate(0x150);
        assert!(elf_load_bytes(&truncated, 0x20_0060, 4).is_err());
    }

    #[test]
    fn source_files_check_hash_size_and_regular_file_before_use() {
        let fixture = Fixture::new();
        let expected = b"synthetic four-byte hash check";
        fs::write(fixture.root.join("small.bin"), expected).unwrap();
        let mut source = SourceFile {
            path: "small.bin".into(),
            sha256: format!("{:X}", Sha256::digest(expected)),
        };
        assert_eq!(
            source_file(&fixture.root, &source, expected.len() as u64).unwrap(),
            expected
        );
        assert!(
            source_file(&fixture.root, &source, expected.len() as u64 - 1)
                .unwrap_err()
                .contains("byte limit")
        );
        source.sha256 = "0".repeat(64);
        assert!(
            source_file(&fixture.root, &source, 100)
                .unwrap_err()
                .contains("fingerprint mismatch")
        );
        source.sha256 = "z".repeat(64);
        assert!(
            source_file(&fixture.root, &source, 100)
                .unwrap_err()
                .contains("Invalid cloud input fingerprint")
        );
        fs::create_dir(fixture.root.join("directory")).unwrap();
        source.path = "directory".into();
        source.sha256 = "0".repeat(64);
        assert!(source_file(&fixture.root, &source, 100).is_err());
    }

    #[test]
    fn traversal_absolute_and_alternate_stream_paths_are_rejected() {
        let fixture = Fixture::new();
        for path in [
            "",
            "../outside.bin",
            "nested/../../outside.bin",
            "/absolute.bin",
            "C:\\outside.bin",
            "\\\\server\\share\\outside.bin",
            "state.bin:stream",
            "bad\0name",
        ] {
            let source = SourceFile {
                path: path.into(),
                sha256: "0".repeat(64),
            };
            assert!(
                source_file(&fixture.root, &source, 100)
                    .unwrap_err()
                    .contains("relative child path"),
                "{path:?}"
            );
            let mut manifest = fixture.boot_manifest();
            manifest["iso_path"] = json!(path);
            assert!(
                fixture.failure(&manifest).contains("relative child path"),
                "{path:?}"
            );
        }
        #[cfg(windows)]
        for path in ["..\\outside.bin", "\\rooted.bin"] {
            let source = SourceFile {
                path: path.into(),
                sha256: "0".repeat(64),
            };
            assert!(
                source_file(&fixture.root, &source, 100)
                    .unwrap_err()
                    .contains("relative child path"),
                "{path:?}"
            );
        }
    }

    #[test]
    fn metadata_rejects_schema_cadence_unknown_draws_and_oversized_json() {
        let fixture = Fixture::new();
        for (field, value) in [
            ("schema_version", json!(2)),
            ("profile", json!("unsupported")),
            ("external_rng_calls_per_tick", json!(17)),
            ("source_state_sha256", json!("bad")),
        ] {
            let mut manifest = fixture.manifest();
            manifest[field] = value;
            assert!(
                fixture
                    .failure(&manifest)
                    .contains("Unsupported cloud checkpoint"),
                "{field}"
            );
        }
        let mut manifest = fixture.manifest();
        manifest["quads"] = json!([]);
        assert!(fixture.failure(&manifest).contains("unknown field"));
        let mut manifest = fixture.manifest();
        manifest["state"]["bytes"] = json!(123);
        assert!(fixture.failure(&manifest).contains("unknown field"));
        let mut manifest = fixture.manifest();
        manifest["emission_checkpoint"] = json!("true");
        assert!(fixture.failure(&manifest).contains("invalid type"));
        let path = fixture.root.join("oversized.json");
        fs::write(&path, vec![b' '; 16_385]).unwrap();
        assert!(
            load_cloud_source(&path, &fixture.root)
                .err()
                .unwrap()
                .contains("byte limit")
        );
    }

    #[test]
    fn disc_boot_manifest_is_validated_before_any_disc_is_opened() {
        let fixture = Fixture::new();
        // A well-formed manifest fails only at the missing ISO.
        assert!(
            fixture
                .failure(&fixture.boot_manifest())
                .contains("not-a-disc.iso")
        );
        let mut manifest = fixture.boot_manifest();
        manifest["schema_version"] = json!(2);
        assert!(
            fixture
                .failure(&manifest)
                .contains("Unsupported cloud checkpoint")
        );
        let mut manifest = fixture.boot_manifest();
        manifest["arithmetic"] = json!("ieee_chop");
        assert!(fixture.failure(&manifest).contains("unknown variant"));
        let mut manifest = fixture.boot_manifest();
        manifest["schedule"]["seed"] = json!(5);
        assert!(fixture.failure(&manifest).contains("unknown field"));
        let mut manifest = fixture.boot_manifest();
        manifest["schedule"]["loop_ticks_before_init"] = json!(u64::MAX);
        assert!(fixture.failure(&manifest).contains("overflow"));
        let mut manifest = fixture.boot_manifest();
        manifest["rng_state_hex"] = json!("0000000000000001");
        assert!(fixture.failure(&manifest).contains("unknown field"));
        // A regular file that is not an ISO is rejected by the disc reader.
        fs::write(fixture.root.join("not-a-disc.iso"), b"synthetic bytes").unwrap();
        let error = fixture.failure(&fixture.boot_manifest());
        assert!(!error.contains("relative child path"), "{error}");
    }

    #[test]
    fn valid_hashes_do_not_bypass_state_table_and_seed_validation() {
        let fixture = Fixture::new();
        for seed in ["", "0x1", "fffffffffffffffff", "gggggggggggggggg"] {
            let mut manifest = fixture.manifest();
            manifest["rng_state_hex"] = json!(seed);
            assert!(fixture.failure(&manifest).contains("sixteen hex digits"));
        }
        // Short but correctly hashed source state must still fail its exact layout.
        fs::write(fixture.root.join("state.bin"), vec![0; 11_599]).unwrap();
        assert!(
            fixture
                .failure(&fixture.manifest())
                .contains("exactly 11,600")
        );
        let fixture = Fixture::new();
        let mut sine = fs::read(fixture.root.join("sine.bin")).unwrap();
        sine[4..8].copy_from_slice(&f32::NAN.to_le_bytes());
        fs::write(fixture.root.join("sine.bin"), sine).unwrap();
        assert!(
            fixture
                .failure(&fixture.manifest())
                .contains("finite, monotone")
        );
    }

    /// Disc-derived start against the exported reference state: the sine
    /// table read through the ELF headers must equal the exported table, and
    /// the boot schedule of the reference recording must replay to the saved
    /// counter-437 bytes and RNG.
    #[test]
    #[ignore = "Requires the user's ISO (ROSES_ISO, workspace-relative) and ROSES_CLOUD_ORACLE_DIR"]
    fn private_disc_boot_source_replays_to_saved_counter437_state() {
        use crate::clouds::{EXTERNAL_RNG_CALLS_PER_TICK, SOURCE_STATE_BYTES, TickInput};
        let root = std::env::var_os("ROSES_WORKSPACE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
        let iso = std::env::var("ROSES_ISO").expect("ROSES_ISO");
        let oracle = PathBuf::from(
            std::env::var_os("ROSES_CLOUD_ORACLE_DIR").expect("ROSES_CLOUD_ORACLE_DIR"),
        );
        let fixture = Fixture::new();
        let manifest = json!({
            "schema_version":1,"profile":DISC_BOOT_PROFILE,"iso_path":iso,
            "schedule":{"loop_ticks_before_init":1222,"flips_before_init":1223},
            "arithmetic":"pcsx2_positive_rounding"
        });
        let path = fixture.root.join("boot.json");
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let mut loaded = load_cloud_source(&path, &root).unwrap();
        assert_eq!(
            loaded.source_state_sha256,
            roses_disc::PAL_EXECUTABLE_SHA256
        );
        assert_eq!(
            loaded.external_rng_calls_per_tick,
            EXTERNAL_RNG_CALLS_PER_TICK
        );
        assert!(!loaded.emission_checkpoint);
        let exported = SineTable::from_le_bytes(
            &fs::read(oracle.join("sine-quarter-1025-f32le.bin")).unwrap(),
        )
        .unwrap();
        for angle in (0..=u16::MAX).step_by(7) {
            assert_eq!(
                loaded.sine.sin(angle).to_bits(),
                exported.sin(angle).to_bits(),
                "{angle}"
            );
        }
        assert_eq!(loaded.rgba_gs.len(), 128 * 64 * 4);
        for _ in 0..437 {
            for _ in 0..loaded.external_rng_calls_per_tick {
                loaded.rng.next_u31();
            }
            loaded
                .state
                .step(&mut loaded.rng, &loaded.sine, TickInput::default())
                .unwrap();
        }
        let saved = fs::read(oracle.join("saved-counter437-state.bin")).unwrap();
        let mine = loaded.state.to_source_bytes();
        assert_eq!(saved.len(), SOURCE_STATE_BYTES);
        assert!((0..SOURCE_STATE_BYTES).all(|i| (8..20).contains(&i) || saved[i] == mine[i]));
        assert_eq!(loaded.rng.state(), 0xdee3_ce4b_ec71_6828);
    }
}
