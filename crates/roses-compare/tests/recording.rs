use roses_compare::recording::{
    companion_state_path, encode_recording, Button, InputSegment, RecordingMetadata, FRAME_BYTES,
    HEADER_BYTES, MAX_FRAMES, MAX_SEGMENTS,
};
use std::path::Path;

fn metadata() -> RecordingMetadata {
    RecordingMetadata {
        emulator_version: "PCSX2-v2.8.2".into(),
        author: "Synthetic protocol test".into(),
        game_name: "Synthetic game".into(),
    }
}

fn segment(frames: u32, buttons: &[Button]) -> InputSegment {
    InputSegment {
        frames,
        buttons: buttons.to_vec(),
    }
}

fn neutral() -> [u8; 18] {
    [
        255, 255, 127, 127, 127, 127, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]
}

#[test]
fn header_uses_exact_v1_offsets_little_endian_counts_and_c_strings() {
    let encoded = encode_recording(&metadata(), &[segment(258, &[])]).unwrap();
    let bytes = encoded.bytes();
    assert_eq!(bytes.len(), 570 + 258 * 36);
    assert_eq!(bytes[0], 1);
    assert_eq!(&bytes[1..13], b"PCSX2-v2.8.2");
    assert!(bytes[13..51].iter().all(|value| *value == 0));
    assert_eq!(&bytes[51..74], b"Synthetic protocol test");
    assert!(bytes[74..306].iter().all(|value| *value == 0));
    assert_eq!(&bytes[306..320], b"Synthetic game");
    assert!(bytes[320..561].iter().all(|value| *value == 0));
    assert_eq!(&bytes[561..565], &[2, 1, 0, 0]);
    assert_eq!(&bytes[565..569], &[0, 0, 0, 0]);
    assert_eq!(bytes[569], 1);
    assert!(!encoded.manifest().replay_verified);
    assert!(encoded.manifest().from_savestate);
}

#[test]
fn frame_intervals_include_exact_press_and_final_release_without_extra_frames() {
    let plan = [
        segment(2, &[]),
        segment(2, &[Button::Start]),
        segment(2, &[]),
    ];
    let encoded = encode_recording(&metadata(), &plan).unwrap();
    assert_eq!(encoded.manifest().total_frames, 6);
    assert_eq!(encoded.bytes().len(), HEADER_BYTES + 6 * FRAME_BYTES);
    let mut pressed = neutral();
    pressed[0] = 0xf7;
    for (frame, expected) in [neutral(), neutral(), pressed, pressed, neutral(), neutral()]
        .iter()
        .enumerate()
    {
        let begin = HEADER_BYTES + frame * FRAME_BYTES;
        assert_eq!(&encoded.bytes()[begin..begin + 18], expected);
        assert_eq!(&encoded.bytes()[begin + 18..begin + 36], &neutral());
    }
    let intervals: Vec<_> = encoded
        .manifest()
        .segments
        .iter()
        .map(|segment| (segment.start_frame, segment.end_frame_exclusive))
        .collect();
    assert_eq!(intervals, [(0, 2), (2, 4), (4, 6)]);
}

#[test]
fn pressure_buttons_have_matching_active_low_bits_and_pressure_channels() {
    for (button, first_byte, second_byte, pressure_index) in [
        (Button::Cross, 255, 191, 12),
        (Button::Circle, 255, 223, 11),
        (Button::Up, 239, 255, 8),
        (Button::Down, 191, 255, 9),
        (Button::Left, 127, 255, 7),
        (Button::Right, 223, 255, 6),
    ] {
        let encoded =
            encode_recording(&metadata(), &[segment(1, &[button]), segment(1, &[])]).unwrap();
        let pad = encoded.manifest().segments[0].port_0_bytes;
        assert_eq!(pad[0], first_byte);
        assert_eq!(pad[1], second_byte);
        assert_eq!(pad[pressure_index], 255);
        assert_eq!(pad[6..].iter().filter(|value| **value != 0).count(), 1);
        assert_eq!(encoded.manifest().segments[1].port_0_bytes, neutral());
    }
}

#[test]
fn input_order_produces_identical_bytes_and_canonical_manifest() {
    let first = encode_recording(
        &metadata(),
        &[segment(1, &[Button::Cross, Button::Up]), segment(1, &[])],
    )
    .unwrap();
    let second = encode_recording(
        &metadata(),
        &[segment(1, &[Button::Up, Button::Cross]), segment(1, &[])],
    )
    .unwrap();
    assert_eq!(first.bytes(), second.bytes());
    assert_eq!(
        serde_json::to_value(first.manifest()).unwrap(),
        serde_json::to_value(second.manifest()).unwrap()
    );
    assert_eq!(first.manifest().segments[0].port_0_bytes[0..2], [239, 191]);
}

#[test]
fn rejects_missing_release_duplicate_buttons_and_opposing_directions() {
    for plan in [
        vec![segment(3, &[Button::Start])],
        vec![segment(1, &[Button::Start, Button::Start]), segment(1, &[])],
        vec![segment(1, &[Button::Up, Button::Down]), segment(1, &[])],
        vec![segment(1, &[Button::Left, Button::Right]), segment(1, &[])],
    ] {
        assert!(encode_recording(&metadata(), &plan).is_err());
    }
}

#[test]
fn rejects_empty_zero_overflow_and_excessive_sequences_before_allocation() {
    for plan in [
        vec![],
        vec![segment(0, &[])],
        vec![segment(MAX_FRAMES + 1, &[])],
        vec![segment(MAX_FRAMES, &[]), segment(1, &[])],
        vec![segment(u32::MAX, &[]), segment(1, &[])],
        vec![segment(1, &[]); MAX_SEGMENTS + 1],
    ] {
        assert!(encode_recording(&metadata(), &plan).is_err());
    }
    assert!(encode_recording(&metadata(), &[segment(1, &[])]).is_ok());
}

#[test]
fn header_text_limits_are_bytes_and_never_silently_truncate() {
    let mut info = metadata();
    info.emulator_version = "x".repeat(49);
    assert!(encode_recording(&info, &[segment(1, &[])]).is_ok());
    info.emulator_version.push('x');
    assert!(encode_recording(&info, &[segment(1, &[])]).is_err());
    info = metadata();
    info.author = "é".repeat(127);
    assert!(encode_recording(&info, &[segment(1, &[])]).is_ok());
    info.author.push('é');
    assert!(encode_recording(&info, &[segment(1, &[])]).is_err());
    info = metadata();
    info.game_name = "embedded\0nul".into();
    assert!(encode_recording(&info, &[segment(1, &[])]).is_err());
    info.game_name.clear();
    assert!(encode_recording(&info, &[segment(1, &[])]).is_err());
}

#[test]
fn companion_filename_appends_after_full_movie_extension() {
    assert_eq!(
        companion_state_path(Path::new("private/movie.p2m2")),
        Path::new("private/movie.p2m2_SaveState.p2s")
    );
}

#[test]
fn writer_never_overwrites_and_does_not_create_or_modify_state_files() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let movie = std::env::temp_dir().join(format!(
        "roses-recording-test-{}-{unique}.p2m2",
        std::process::id()
    ));
    let companion = companion_state_path(&movie);
    let encoded = encode_recording(&metadata(), &[segment(1, &[])]).unwrap();
    encoded.write_new(&movie).unwrap();
    let bytes_before = std::fs::read(&movie).unwrap();
    assert!(encoded.write_new(&movie).is_err());
    assert_eq!(std::fs::read(&movie).unwrap(), bytes_before);
    assert_eq!(bytes_before, encoded.bytes());
    assert!(!companion.exists());
    std::fs::remove_file(movie).unwrap();
}
