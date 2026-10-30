//! The duel's music and sound effects from `roses import-duel-audio`
//! (`claude-audio/DUEL-AUDIO-SCOPE.md`): music tracks decoded on demand, the
//! system (slot 0) and card (slot 1) banks, and the two battling monsters'
//! groups (slots 2 / 3, SEGRP.MRG slot = card id, 0x80800 bytes each),
//! ready for `roses_client::sound_mix::Mixer`.
use roses_client::sound_mix::{MusicPcm, SeSample};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const MANIFEST_SCHEMA: &str = "roses-duel-audio-import-1";
const SEGRP_SLOT_BYTES: u64 = 0x80800;

/// What the duel asks of the sound driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioCue {
    /// 0x217110(track, mode): mode 0 fades the current track first, mode 1
    /// keeps the track when it already plays.
    Music { track: u32, keep: bool },
    /// 0x2171A0(speed).
    FadeOut { speed: i32 },
    /// 0x216EA0(slot, id); `skip`: id bit 0x8000 (dropped while playing).
    Se { slot: u8, id: u8, skip: bool },
    /// 0x216D20(slot, card group): load a monster's group into slot 2 / 3.
    Group { slot: u8, card: u16 },
    /// 0x217050(slot, id): stop the entry's playing voices.
    StopSe { slot: u8, id: u8 },
}

#[derive(Deserialize)]
struct FileRef {
    path: String,
    sha256: String,
}
#[derive(Deserialize)]
struct TrackRef {
    track: u32,
    path: String,
    sha256: String,
    #[serde(rename = "loop")]
    looped: Option<[u32; 2]>,
}
#[derive(Deserialize)]
struct Bank {
    file: FileRef,
}
#[derive(Deserialize)]
struct Banks {
    system: Bank,
    card: Bank,
    monster_groups: Bank,
}
#[derive(Deserialize)]
struct Values {
    values: Vec<u16>,
}
#[derive(Deserialize)]
struct Manifest {
    schema: String,
    pcmlist: Option<FileRef>,
    tracks: Vec<TrackRef>,
    banks: Banks,
    pitch_table: Values,
}

pub struct DuelAudio {
    root: PathBuf,
    tracks: BTreeMap<u32, (String, String, Option<[u32; 2]>)>,
    /// Track ids the disc's PCMLIST.BIN lists (`None`: no PCMLIST read). A
    /// request for any other id (track 1 at 11124) only fades the playing
    /// track out in the original [F UI-SOUNDS.md 2.4]: not a missing import.
    disc_tracks: Option<std::collections::BTreeSet<u32>>,
    decoded: BTreeMap<u32, Arc<MusicPcm>>,
    pitch: Vec<u16>,
    /// Slots 0..3: the bank's entries.
    banks: [Vec<SeSample>; 4],
    groups: [Option<u16>; 2],
    segrp: (String, String),
    pub reported: std::collections::BTreeSet<String>,
}

fn read_checked(root: &Path, path: &str, sha: &str) -> Result<Vec<u8>, String> {
    if path.contains("..") || path.contains(':') || path.starts_with('/') {
        return Err(format!("audio path {path} leaves the import"));
    }
    let bytes = std::fs::read(root.join(path)).map_err(|e| format!("{path}: {e}"))?;
    if format!("{:x}", Sha256::digest(&bytes)) != sha {
        return Err(format!("{path} does not match its import hash"));
    }
    Ok(bytes)
}

fn samples(entries: Vec<roses_assets::sound::SoundEffect>, pitch: &[u16]) -> Vec<SeSample> {
    entries
        .into_iter()
        .map(|e| SeSample {
            pitch: roses_assets::sound::note_pitch(pitch, e.note, e.fine),
            samples: e.samples.into(),
            loop_start: e.loop_start,
            volume: e.volume,
            priority: e.priority,
            voice_group: e.voice_group,
        })
        .collect()
}

impl DuelAudio {
    pub fn load(manifest_path: &Path) -> Result<Self, String> {
        let bytes = crate::bounded_read(manifest_path, 4 * 1024 * 1024)?;
        let m: Manifest = serde_json::from_slice(&bytes).map_err(|e| format!("audio manifest: {e}"))?;
        if m.schema != MANIFEST_SCHEMA {
            return Err(format!("unsupported audio import {}", m.schema));
        }
        let root = manifest_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        if m.pitch_table.values.len() != 12 * 128 {
            return Err("audio pitch table has the wrong size".into());
        }
        let bank = |b: &Bank| -> Result<Vec<SeSample>, String> {
            let bytes = read_checked(&root, &b.file.path, &b.file.sha256)?;
            Ok(samples(roses_assets::sound::decode_grp(&bytes).map_err(|e| e.to_string())?, &m.pitch_table.values))
        };
        let disc_tracks = m
            .pcmlist
            .as_ref()
            .and_then(|f| read_checked(&root, &f.path, &f.sha256).ok())
            .and_then(|b| roses_assets::sound::parse_pcmlist(&b).ok())
            .map(|records| records.iter().map(|r| r.track).collect());
        let system = bank(&m.banks.system)?;
        let card = bank(&m.banks.card)?;
        Ok(Self {
            tracks: m.tracks.iter().map(|t| (t.track, (t.path.clone(), t.sha256.clone(), t.looped))).collect(),
            disc_tracks,
            decoded: BTreeMap::new(),
            pitch: m.pitch_table.values,
            banks: [system, card, Vec::new(), Vec::new()],
            groups: [None; 2],
            segrp: (m.banks.monster_groups.file.path, m.banks.monster_groups.file.sha256),
            root,
            reported: Default::default(),
        })
    }

    fn report(&mut self, what: String) {
        if self.reported.insert(what.clone()) {
            eprintln!("Duel audio: unsupported, not played: {what}");
        }
    }

    /// A track decoded elsewhere (the story import's pinned history and
    /// Simon tracks 24 / 25).
    pub fn insert_track(&mut self, pcm: Arc<MusicPcm>) {
        self.decoded.insert(pcm.track, pcm);
    }

    pub fn has_track(&self, track: u32) -> bool {
        self.decoded.contains_key(&track) || self.tracks.contains_key(&track)
    }

    /// A music track, decoded once.
    pub fn track(&mut self, track: u32) -> Option<Arc<MusicPcm>> {
        if let Some(t) = self.decoded.get(&track) {
            return Some(t.clone());
        }
        let Some((path, sha, looped)) = self.tracks.get(&track).cloned() else {
            if self.disc_tracks.as_ref().is_some_and(|t| !t.contains(&track)) {
                // Not on the disc: the original's request is a fade-out.
                return None;
            }
            self.report(format!("music track {track} (not in the import)"));
            return None;
        };
        let bytes = match read_checked(&self.root, &path, &sha) {
            Ok(b) => b,
            Err(e) => {
                self.report(e);
                return None;
            }
        };
        let frames = u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?) as usize;
        let data = bytes.get(2048..2048 + frames * 4)?;
        let pcm = Arc::new(MusicPcm {
            track,
            samples: data.chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]])).collect::<Vec<_>>().into(),
            looped: looped.map(|[a, b]| (a as usize, b as usize)),
        });
        self.decoded.insert(track, pcm.clone());
        Some(pcm)
    }

    /// Bank entry `id` of slot 0..3.
    pub fn se(&mut self, slot: u8, id: u8) -> Option<SeSample> {
        let s = self.banks.get(usize::from(slot))?.get(usize::from(id)).cloned();
        if s.is_none() {
            self.report(format!("sound effect ({slot}, {id:#x}) (no such bank entry)"));
        }
        s
    }

    /// 0x216D20(2 + which, card): SEGRP.MRG slot `card`.
    pub fn load_group(&mut self, slot: u8, card: u16) {
        let which = usize::from(slot.saturating_sub(2)).min(1);
        if self.groups[which] == Some(card) {
            return;
        }
        self.groups[which] = Some(card);
        self.banks[2 + which] = Vec::new();
        use std::io::{Read, Seek, SeekFrom};
        let path = self.root.join(&self.segrp.0);
        let mut bytes = vec![0u8; SEGRP_SLOT_BYTES as usize];
        let read = std::fs::File::open(&path).and_then(|mut f| {
            f.seek(SeekFrom::Start(u64::from(card) * SEGRP_SLOT_BYTES))?;
            f.read_exact(&mut bytes)
        });
        if read.is_err() {
            self.report(format!("monster sound group of card {card} (SEGRP.MRG unreadable)"));
            return;
        }
        if bytes[..8].iter().all(|b| *b == 0) {
            return;
        }
        match roses_assets::sound::decode_grp(&bytes) {
            Ok(entries) => self.banks[2 + which] = samples(entries, &self.pitch),
            Err(e) => self.report(format!("monster sound group of card {card}: {e}")),
        }
    }
}
