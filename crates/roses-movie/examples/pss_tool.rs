//! Research/verification tool for roses-movie (not shipped in the game).
//!
//!   pss_tool inventory --iso <iso>
//!   pss_tool extract   --iso <iso> --file MOVIE/X.PSS --out <new.pss>
//!   pss_tool stats     --pss <file>
//!   pss_tool dump-audio --pss <file> --out <new.wav> [--ss2 <new.ss2>]
//!   pss_tool compare-audio --pss <file> --reference <raw s16le stereo file>
//!
//! No external program is run: every check here uses only roses-movie.
use roses_movie::movie::{decode_audio, MovieReader};
use roses_movie::ps::{PsReader, STREAM_PRIVATE_1, STREAM_VIDEO_0};
use std::collections::BTreeMap;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::time::Instant;

type R<T> = Result<T, Box<dyn std::error::Error>>;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() -> R<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("inventory") => inventory(&arg(&args, "--iso").ok_or("--iso")?),
        Some("extract") => {
            let mut disc = roses_disc::Disc::open(arg(&args, "--iso").ok_or("--iso")?)?;
            let bytes = disc.read_file(&arg(&args, "--file").ok_or("--file")?, 512 << 20)?;
            let out = PathBuf::from(arg(&args, "--out").ok_or("--out")?);
            if out.exists() {
                return Err("output exists".into());
            }
            std::fs::write(out, bytes)?;
            Ok(())
        }
        Some("stats") => {
            let path = arg(&args, "--pss").ok_or("--pss")?;
            let bytes = std::fs::read(&path)?;
            println!("{}", stats(&path, &bytes)?);
            Ok(())
        }
        Some("bench") => {
            // Decode + RGBA conversion of every frame (the player thread's work).
            let path = arg(&args, "--pss").ok_or("--pss")?;
            let t = Instant::now();
            let mut r = MovieReader::new(BufReader::new(std::fs::File::open(&path)?));
            let (mut n, mut rgba_time) = (0u64, 0f64);
            let mut rgba = Vec::new();
            while let Some(f) = r.next_frame()? {
                let c = Instant::now();
                rgba.resize(f.width * f.height * 4, 0);
                f.to_rgba(&mut rgba);
                rgba_time += c.elapsed().as_secs_f64();
                n += 1;
                let _ = r.take_audio();
            }
            let total = t.elapsed().as_secs_f64();
            let a = Instant::now();
            let (_, pcm) = decode_audio(BufReader::new(std::fs::File::open(&path)?))?;
            println!(
                "{n} frames in {total:.2} s ({:.0} fps, {:.2} ms/frame; RGBA {:.2} ms/frame); audio-only pass {:.3} s for {} samples",
                n as f64 / total,
                total * 1000.0 / n as f64,
                rgba_time * 1000.0 / n as f64,
                a.elapsed().as_secs_f64(),
                pcm.len()
            );
            Ok(())
        }
        Some("dump-audio") => dump_audio(&args),
        Some("compare-audio") => compare_audio(&args),
        _ => Err("see the file header for usage".into()),
    }
}

fn inventory(iso: &str) -> R<()> {
    let mut disc = roses_disc::Disc::open(iso)?;
    let entries: Vec<_> = disc
        .entries()
        .iter()
        .filter(|e| !e.is_directory && e.path.to_ascii_uppercase().ends_with(".PSS"))
        .map(|e| (e.path.clone(), e.size))
        .collect();
    println!("{} PSS files", entries.len());
    for (path, size) in entries {
        println!("== {path} ({size} bytes)");
        let bytes = disc.read_file(&path, 512 << 20)?;
        match stats(&path, &bytes) {
            Ok(text) => println!("{text}"),
            Err(e) => println!("  FAILED: {e}"),
        }
    }
    Ok(())
}

/// Container survey plus a full strict decode of video and audio.
fn stats(name: &str, bytes: &[u8]) -> R<String> {
    let mut s = String::new();
    use std::fmt::Write as _;
    // Container: streams, packet counts, PTS ranges.
    let mut ps = PsReader::new(bytes);
    let mut streams: BTreeMap<(u8, Option<u8>), (u64, u64, Option<u64>, Option<u64>, u64)> =
        BTreeMap::new();
    let mut video_pts = Vec::new();
    while let Some(p) = ps.next_packet()? {
        let e = streams.entry((p.stream_id, p.substream)).or_insert((0, 0, None, None, 0));
        e.0 += 1;
        e.1 += p.payload.len() as u64;
        if let Some(t) = p.pts {
            e.2 = Some(e.2.map_or(t, |v: u64| v.min(t)));
            e.3 = Some(e.3.map_or(t, |v: u64| v.max(t)));
            e.4 += 1;
            if p.stream_id == STREAM_VIDEO_0 {
                video_pts.push(t);
            }
        }
    }
    writeln!(s, "file {name}: {} bytes, end code {}", bytes.len(), ps.saw_end_code())?;
    for ((id, sub), (n, b, lo, hi, np)) in &streams {
        writeln!(
            s,
            "  stream {id:02X}{} packets {n} payload {b} bytes, PTS {np}x {:?}..{:?}",
            sub.map(|v| format!("/{v:02X}")).unwrap_or_default(),
            lo,
            hi
        )?;
    }
    video_pts.sort();
    let steps: BTreeMap<u64, u64> = video_pts.windows(2).fold(BTreeMap::new(), |mut m, w| {
        *m.entry(w[1] - w[0]).or_insert(0) += 1;
        m
    });
    writeln!(s, "  video PTS steps (90 kHz): {steps:?}")?;
    let _ = STREAM_PRIVATE_1;
    // Decode.
    let start = Instant::now();
    let mut reader = MovieReader::new(bytes);
    reader.set_strict(true);
    let mut frames = 0u64;
    let mut types = [0u64; 3];
    let mut audio = 0usize;
    loop {
        let f = match reader.next_frame() {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                return Err(format!(
                    "{e} after {frames} frames; seq {:?}; stats {:?}",
                    reader.sequence(),
                    reader.video_stats()
                )
                .into())
            }
        };
        frames += 1;
        types[match f.picture_type {
            b'I' => 0,
            b'P' => 1,
            _ => 2,
        }] += 1;
        audio += reader.take_audio().len();
    }
    loop {
        audio += reader.take_audio().len();
        if !reader.pump()? {
            break;
        }
    }
    audio += reader.take_audio().len();
    let secs = start.elapsed().as_secs_f64();
    let info = reader.info().clone();
    let seq = reader.sequence().cloned();
    writeln!(s, "  sequence: {seq:?}")?;
    writeln!(s, "  video stats: {:?}", reader.video_stats())?;
    writeln!(
        s,
        "  decoded {frames} frames (I {} P {} B {}), {} s at {}/{} fps = {:.3} s; decode took {:.2} s ({:.1} fps)",
        types[0],
        types[1],
        types[2],
        frames,
        info.frame_rate.0,
        info.frame_rate.1,
        frames as f64 * f64::from(info.frame_rate.1) / f64::from(info.frame_rate.0.max(1)),
        secs,
        frames as f64 / secs
    )?;
    if let Some(a) = info.audio {
        let per = audio as u64 / u64::from(a.channels);
        writeln!(
            s,
            "  audio: {a:?}; {per} samples/channel = {:.3} s; first PTS video {:?} audio {:?}",
            per as f64 / f64::from(a.sample_rate),
            info.first_video_pts,
            info.first_audio_pts
        )?;
    } else {
        writeln!(s, "  audio: none")?;
    }
    Ok(s)
}

fn dump_audio(args: &[String]) -> R<()> {
    let pss = arg(args, "--pss").ok_or("--pss")?;
    let out = PathBuf::from(arg(args, "--out").ok_or("--out")?);
    let bytes = std::fs::read(&pss)?;
    if let Some(ss2) = arg(args, "--ss2") {
        // The raw SShd/SSbd stream (substream tags removed), for an
        // independent decode by another tool.
        let mut ps = PsReader::new(&bytes[..]);
        let mut raw = Vec::new();
        while let Some(p) = ps.next_packet()? {
            if p.stream_id == STREAM_PRIVATE_1 && p.substream == Some(0xA0) {
                raw.extend_from_slice(&p.payload);
            }
        }
        std::fs::write(ss2, raw)?;
    }
    let t = Instant::now();
    let (header, pcm) = decode_audio(&bytes[..])?;
    let header = header.ok_or("no audio")?;
    println!(
        "{header:?}: {} samples/channel in {:.3} s",
        pcm.len() / header.channels as usize,
        t.elapsed().as_secs_f64()
    );
    let mut f = std::fs::File::create_new(out)?;
    let data = (pcm.len() * 2) as u32;
    let ch = header.channels as u16;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&ch.to_le_bytes())?;
    f.write_all(&header.sample_rate.to_le_bytes())?;
    f.write_all(&(header.sample_rate * u32::from(ch) * 2).to_le_bytes())?;
    f.write_all(&(ch * 2).to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data.to_le_bytes())?;
    let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
    f.write_all(&bytes)?;
    Ok(())
}

/// Ours vs a raw s16le stereo reference: exact match count, then the best
/// alignment (for codecs with priming delay) and SNR at that alignment.
fn compare_audio(args: &[String]) -> R<()> {
    let pss = arg(args, "--pss").ok_or("--pss")?;
    let reference = std::fs::read(arg(args, "--reference").ok_or("--reference")?)?;
    let (_, ours) = decode_audio(BufReader::new(std::fs::File::open(&pss)?))?;
    let theirs: Vec<i16> = reference
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();
    println!(
        "samples (interleaved): ours {} reference {}; per channel ours {} reference {}",
        ours.len(),
        theirs.len(),
        ours.len() / 2,
        theirs.len() / 2
    );
    let same = ours.iter().zip(&theirs).filter(|(a, b)| a == b).count();
    println!("identical at zero offset: {same} of {}", ours.len().min(theirs.len()));
    // Search offsets (in stereo frames) on a 10 s window around 30 s.
    let window = 48_000 * 10;
    let at = 48_000 * 30;
    let mut best = (i64::MIN, f64::MAX);
    for off in -4096i64..=4096 {
        let mut se = 0f64;
        for i in (at..at + window).step_by(7) {
            let j = i as i64 + off;
            if j < 0 || (j as usize) * 2 + 1 >= theirs.len() || i * 2 + 1 >= ours.len() {
                continue;
            }
            let d = f64::from(ours[i * 2]) - f64::from(theirs[j as usize * 2]);
            se += d * d;
        }
        if se < best.1 {
            best = (off, se);
        }
    }
    let off = best.0;
    let (mut sig, mut err, mut n) = (0f64, 0f64, 0u64);
    let mut peak_diff = 0i32;
    for i in 0..ours.len() / 2 {
        let j = i as i64 + off;
        if j < 0 || (j as usize) * 2 + 1 >= theirs.len() {
            continue;
        }
        for c in 0..2 {
            let a = f64::from(ours[i * 2 + c]);
            let b = f64::from(theirs[j as usize * 2 + c]);
            sig += a * a;
            err += (a - b) * (a - b);
            peak_diff = peak_diff.max((a - b).abs() as i32);
        }
        n += 1;
    }
    println!(
        "best alignment: reference = ours shifted by {off} samples; over {n} stereo frames SNR {:.2} dB, RMS ours {:.1}, RMS error {:.2}, peak |diff| {peak_diff}",
        10.0 * (sig / err.max(1e-9)).log10(),
        (sig / (2 * n) as f64).sqrt(),
        (err / (2 * n) as f64).sqrt()
    );
    // Coarse envelope comparison: RMS per 100 ms block, correlation.
    let block = 4800usize;
    let blocks = (ours.len() / 2) / block;
    let (mut sa, mut sb, mut sab, mut saa, mut sbb, mut k) = (0f64, 0f64, 0f64, 0f64, 0f64, 0f64);
    for bi in 0..blocks {
        let (mut ea, mut eb) = (0f64, 0f64);
        for i in bi * block..(bi + 1) * block {
            let j = i as i64 + off;
            if j < 0 || (j as usize) * 2 + 1 >= theirs.len() {
                continue;
            }
            ea += f64::from(ours[i * 2]).powi(2) + f64::from(ours[i * 2 + 1]).powi(2);
            eb += f64::from(theirs[j as usize * 2]).powi(2) + f64::from(theirs[j as usize * 2 + 1]).powi(2);
        }
        let (ea, eb) = (ea.sqrt(), eb.sqrt());
        sa += ea;
        sb += eb;
        sab += ea * eb;
        saa += ea * ea;
        sbb += eb * eb;
        k += 1.0;
    }
    let cov = sab / k - sa / k * sb / k;
    let corr = cov / ((saa / k - (sa / k).powi(2)).sqrt() * (sbb / k - (sb / k).powi(2)).sqrt());
    println!("100 ms RMS envelope correlation over {blocks} blocks: {corr:.6}");
    Ok(())
}
