//! Private checks against the player's own disc files. Set ROSES_PSS_DIR to a
//! directory holding the PSS files extracted from the PAL ISO (for example
//! with `cargo run --example pss_tool -- extract ...`). Skipped otherwise.
//!
//! Expected values: frame and sample counts from the container (PTS count,
//! SSbd size); the digests are of this decoder's output, which was verified
//! bit-exact (all frames, all planes) against an independent MPEG-2 decoder
//! using the same integer IDCT, and within +-3 (>= 62 dB per frame) with the
//! reference IDCT used here. See work/research/movie/claude-pss/NOTES.md.
use roses_movie::MovieReader;
use std::path::PathBuf;

fn dir() -> Option<PathBuf> {
    std::env::var_os("ROSES_PSS_DIR").map(PathBuf::from)
}

fn fnv(hash: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *hash ^= u64::from(b);
        *hash = hash.wrapping_mul(0x100_0000_01B3);
    }
}

fn check(name: &str, frames: u64, samples_per_channel: u64, digest: Option<u64>) {
    let Some(dir) = dir() else {
        eprintln!("ROSES_PSS_DIR not set; skipping {name}");
        return;
    };
    let file = std::fs::File::open(dir.join(name)).unwrap();
    let mut r = MovieReader::new(std::io::BufReader::new(file));
    r.set_strict(true);
    let (mut n, mut audio, mut hash) = (0u64, 0usize, 0xCBF2_9CE4_8422_2325u64);
    while let Some(f) = r.next_frame().unwrap() {
        assert_eq!((f.width, f.height, f.display_index), (640, 448, n));
        fnv(&mut hash, &f.y);
        fnv(&mut hash, &f.cb);
        fnv(&mut hash, &f.cr);
        n += 1;
        audio += r.take_audio().len();
    }
    audio += r.take_audio().len();
    assert!(r.ended_cleanly());
    assert_eq!(r.video_stats().errors, 0);
    assert_eq!(n, frames);
    assert_eq!(audio as u64 / 2, samples_per_channel);
    let a = r.info().audio.unwrap();
    assert_eq!((a.sample_rate, a.channels), (48_000, 2));
    println!("{name}: digest {hash:#018x}");
    if let Some(d) = digest {
        assert_eq!(hash, d, "{name} decoded output changed");
    }
}

#[test]
fn opening_movie() {
    check("OPMV_EU.PSS", 2867, 5_504_768, DIGESTS[0]);
}
#[test]
fn ending_movie_a6() {
    check("EDMV_A6.PSS", 3000, 5_760_000, DIGESTS[1]);
}
#[test]
fn ending_movie_c6() {
    check("EDMV_C6.PSS", 3000, 5_760_000, DIGESTS[2]);
}
#[test]
fn dummy_movie() {
    check("DUMMY.PSS", 330, 528_128, DIGESTS[3]);
}

// OPMV_EU, EDMV_A6, EDMV_C6, DUMMY (reference IDCT build).
const DIGESTS: [Option<u64>; 4] = [
    Some(0x29ef_602b_5604_b93c),
    Some(0xe71b_2a1c_9ebd_26b7),
    Some(0x8873_34e3_8201_ee7f),
    Some(0x7a67_06b6_4132_02bf),
];
