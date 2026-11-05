//! `roses import-duel-audio`: the duel's music and sound effects from the
//! user's ISO into a new private directory, copied verbatim and checked by
//! decoding them natively (`roses_assets::sound`; no external tools):
//! `PCMLIST.BIN` and every music track it lists (the duel's start tracks,
//! battle tracks 13-22, the post-battle variants k * 1000 + the duel's
//! track, rewards 21, win 23, lose 42, and every other scene's tracks; a
//! cued id PCMLIST lacks, track 1, is the original's fade to silence), the
//! system and card SE banks (`SYSTEM0.GRP`,
//! `CARD00.GRP`), the monster SE groups (`SEGRP.MRG`), and from the disc's
//! code the IOP driver's pitch table (`KCEJEAST.IRX`) and the card -> SE
//! group table (EE 0x2CE024). Nothing here may be distributed.
//!
//! Traced in the project notes.
use crate::{emit_json, open_pal, Result};
use roses_client::campaign_map::EE_TO_FILE;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub const SCHEMA: &str = "roses-duel-audio-import-1";
/// Tracks a duel plays (DUEL-AUDIO-SCOPE.md 3.2 / 3.3) and the tracks of
/// the scenes around it: title 2, campaign map 4, deck editor 5, Seto 26,
/// Brest 27, the red route's events 28-34 (UI-SOUNDS.md 7.4).
/// These are all imported, and since the post-battle variants of every
/// duel track (1009..3044, 0x1AB440(c84) with c84 % 1000 = 1) and the
/// other scenes' tracks are cued too, so is every other PCMLIST track.
pub const DUEL_TRACKS: [u32; 41] = [
    7, 8, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 42, 1008, 2008, 3008, 9, 2, 4, 5, 26, 27, 28, 29, 30, 31, 32, 33, 34,
    // Other opponents' duel tracks (0x1AB440 code 1), Custom Duel / Trade
    // menus, the white route.
    10, 11, 12, 40, 44, 3, 41, 35, 36, 37, 38,
];
const GROUP_TABLE_EE: u32 = 0x002C_E024;
const GROUP_TABLE_ENTRIES: u32 = 854;

fn file_entry(output: &Path, name: &str, bytes: &[u8]) -> Result<serde_json::Value> {
    fs::write(output.join(name), bytes)?;
    Ok(serde_json::json!({"path": name, "sha256": format!("{:x}", Sha256::digest(bytes)), "bytes": bytes.len()}))
}

pub fn run(iso: &Path, output: &Path) -> Result<()> {
    if output.exists() {
        return Err("duel audio output must be a new directory; existing imports are never overwritten".into());
    }
    let (mut disc, report) = open_pal(iso)?;
    let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024)?;
    let pcmlist = disc.read_file("SOUND/BGM/PCMLIST.BIN", 64 * 1024)?;
    let records = roses_assets::sound::parse_pcmlist(&pcmlist)?;
    let system = disc.read_file("SOUND/SYSTEM/SYSTEM0.GRP", 4 * 1024 * 1024)?;
    let card = disc.read_file("SOUND/CARD/CARD00.GRP", 4 * 1024 * 1024)?;
    let irx = disc.read_file("IOP/KCEJEAST.IRX", 4 * 1024 * 1024)?;
    let pitch = roses_assets::sound::parse_pitch_table(&irx)?;
    let system_entries = roses_assets::sound::decode_grp(&system)?.len();
    let card_entries = roses_assets::sound::decode_grp(&card)?.len();
    let segrp = disc.read_file("SOUND/MONSTER/SEGRP.MRG", 512 * 1024 * 1024)?;
    let groups: Vec<u16> = (0..GROUP_TABLE_ENTRIES)
        .map(|i| {
            let at = (GROUP_TABLE_EE + 2 * i - EE_TO_FILE) as usize;
            executable
                .get(at..at + 2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .ok_or_else(|| "executable too short for the SE group table".into())
        })
        .collect::<Result<_>>()?;
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    let mut tracks = Vec::new();
    let mut missing = Vec::new();
    let mut ids: Vec<u32> = DUEL_TRACKS.to_vec();
    let mut others: Vec<u32> = records
        .iter()
        .map(|r| r.track)
        .filter(|t| !DUEL_TRACKS.contains(t))
        .collect();
    others.sort_unstable();
    others.dedup();
    ids.extend(others);
    for id in ids {
        let Some(record) = records.iter().find(|r| r.track == id) else {
            missing.push(id);
            continue;
        };
        let path = roses_assets::sound::track_path(id);
        let bytes = disc.read_file(&path, 64 * 1024 * 1024)?;
        let decoded = roses_assets::sound::decode_track(&bytes, record)?;
        let name = format!("{id:04}.PCM");
        let mut entry = file_entry(output, &name, &bytes)?;
        entry["track"] = id.into();
        entry["frames"] = (decoded.samples.len() / 2).into();
        entry["loop"] = match decoded.looped {
            Some((a, b)) => serde_json::json!([a, b]),
            None => serde_json::Value::Null,
        };
        tracks.push(entry);
    }
    emit_json(&report, Some(&output.join("disc.json")))?;
    emit_json(
        &serde_json::json!({
            "schema": SCHEMA,
            "importer_version": env!("CARGO_PKG_VERSION"),
            "profile": roses_disc::PAL_PROFILE_ID,
            "source": {"iso_sha256": report.iso_sha256, "executable_sha256": format!("{:x}", Sha256::digest(&executable))},
            "pcmlist": file_entry(output, "PCMLIST.BIN", &pcmlist)?,
            "tracks": tracks,
            "tracks_missing": missing,
            "banks": {
                "system": {"file": file_entry(output, "SYSTEM0.GRP", &system)?, "entries": system_entries},
                "card": {"file": file_entry(output, "CARD00.GRP", &card)?, "entries": card_entries},
                "monster_groups": {"file": file_entry(output, "SEGRP.MRG", &segrp)?, "slot_bytes": 0x80800},
            },
            "group_table": {"ee": GROUP_TABLE_EE, "values": groups},
            "pitch_table": {"source": "IOP/KCEJEAST.IRX .rodata 0x13160", "values": pitch},
            "not_included": [
                "SECTL.MRG (battle-scene sound control records, structure not decoded)",
                "music track ids PCMLIST.BIN does not list (track 1, cued at 11124: the original fades to silence)"
            ],
        }),
        Some(&output.join("manifest.json")),
    )?;
    Ok(())
}
