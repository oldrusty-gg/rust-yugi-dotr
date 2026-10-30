# Deterministic input recording generation

`recording::encode_recording` generates PCSX2 `.p2m2` files using **format version
1 as implemented in PCSX2 v2.8.2**. It serializes an explicit controller sequence
into bytes. It does not communicate with the emulator, issue operating-system
keystrokes, load a state, or write guest memory. Live playback and repeatability
are separate validation steps; the limited local playback evidence below does
not establish repeatability or native-game fidelity.

```rust,no_run
use roses_compare::recording::{
    encode_recording, companion_state_path, Button, InputSegment, RecordingMetadata,
};
use std::path::Path;

let metadata = RecordingMetadata {
    emulator_version: "PCSX2-v2.8.2".into(),
    author: "Roses reference research".into(),
    game_name: "User-supplied game title".into(),
};
let segments = [
    InputSegment { frames: 60, buttons: vec![] },
    InputSegment { frames: 3, buttons: vec![Button::Start] },
    InputSegment { frames: 60, buttons: vec![] },
];
let recording = encode_recording(&metadata, &segments)?;
let movie = Path::new("private-working-copy/movie.p2m2");
recording.write_new(movie)?;
let required_state = companion_state_path(movie);
// Caller must copy a compatible, preserved state to required_state explicitly.
// Preserve source/working-copy hashes and the serialized recording.manifest().
# Ok::<(), Box<dyn std::error::Error>>(())
```

The output stores 123 frame rows in this example: neutral at indices 0–59,
Start at 60–62, neutral at 63–122. The file's frame count is 123. There is no
implicit extra frame, and each interval in the manifest uses an exclusive end.
The caller must provide an explicit final neutral segment so the release is
represented inside the recording. This requirement rejects a held final button
instead of silently changing the requested duration.

The supported buttons are `start`, `cross`, `circle`, `up`, `down`, `left`, and
`right`. Empty button lists represent neutral input. Segment lengths must be
positive; duplicate buttons and opposing directions are rejected. The generator
caps sequences at 4096 segments and 1,000,000 frames before allocating the output.
Button order is canonicalized so equivalent combinations produce identical bytes
and manifests. Controller port 1 remains neutral throughout, and analog actions
and multitap are unsupported.

`RecordingMetadata` and `InputSegment` support JSON deserialization with unknown
fields rejected. Header strings are validated by UTF-8 byte length, must contain
no embedded NUL, and are never silently truncated. `EncodedRecording::bytes()`
exposes an immutable byte slice. Its manifest records exact segment intervals,
button lists, raw bytes for both controller ports, frame/byte counts, source
release and explicit `replay_verified: false`.

## Preserved states and working copies

All generated movies start from a savestate. For `movie.p2m2`, PCSX2 expects the
companion filename **`movie.p2m2_SaveState.p2s`**, retaining the movie extension.
The generator sets the savestate flag but neither manufactures a state nor
authenticates one. `companion_state_path` only computes the required path.

Keep the original state and generated movie as archived evidence with hashes.
Copy both into a new playback directory, retaining the required naming
relationship. The caller should record the exact emulator release, disc,
configuration, source-state hash and movie hash alongside the input manifest.
The generator's `write_new` refuses to replace an existing movie and synchronizes
its contents; failed I/O can leave a partial new file, which must not be treated
as a completed recording.

Do not assume an emulator-opened movie remains immutable. PCSX2 v2.8.2 opens
existing recordings for reading and writing. Its replay counter requests a pause
when it reaches the stored total; continuing beyond the end can switch into
recording mode and alter the working file. Therefore use disposable copies and
compare their hashes after a run. Making a movie filesystem-read-only is not the
substitute for keeping archived originals, because the emulator opens it with
write access. These behaviors and the companion path follow the pinned
[recording implementation](https://github.com/PCSX2/pcsx2/blob/v2.8.2/pcsx2/Recording/InputRecording.cpp)
and [file access implementation](https://github.com/PCSX2/pcsx2/blob/v2.8.2/pcsx2/Recording/InputRecordingFile.cpp).

## Observed local playback and endpoint behavior

On 27 September 2026, the local PCSX2 v2.8.2 release accepted generated files and
restored their preserved PAL main-menu state. Two 120-frame runs produced
different visible selections as intended by their inputs: neutral input left
NEW GAME selected; neutral 10 / Down 10 / neutral 100 selected CONTINUE. Both
automatically paused at the displayed endpoint of 120. This establishes the
recorded controller path for this setup, not repeated-identical-input
determinism or agreement with a native rules engine.

The writable copies grew from **4,890 to 4,926 bytes**, adding one physical
36-byte row, while their archived originals remained unchanged. Their header
frame count stayed **120**, so the extra physical record is not evidence of a
121-frame simulation. The earlier Cross-input
probe likewise appended one neutral frame at its endpoint, with log entries for
pausing and record mode. Thus automatic pause did not preserve the playback file
byte-for-byte in this build. The generator continues to emit exactly the
requested number of rows; it does not add padding to imitate this runtime
extension. Never treat a modified working copy as the original input evidence.

These observations are specific to the tested release and local configuration.
MTVU was enabled during the experiment, and its logs warned about savestate
stability. The private report
`work/reference/recordings/menu-input-experiment.json` records source-state
and endpoint hashes and the limits of the experiment. It is not shipped with
the source repository. Generated manifests remain `replay_verified: false`;
observations of a particular run belong in a separate evidence report.

### Switching recordings and stale overlays

The v2.8.2 GUI playback handler stops an active recording before starting the
next one, so its ordinary Play action already performs that lifecycle step.
[MainWindow.cpp](https://github.com/PCSX2/pcsx2/blob/v2.8.2/pcsx2-qt/MainWindow.cpp)
contains this sequence. Explicitly stopping while paused remains a useful way
to make experiment logs and working-file closure clear.

The core playback method sets replay mode before resetting its frame counter to
zero. When paused, changing mode can present a frame immediately. This ordering
provides a source-based explanation for briefly seeing a previous counter with
the new movie's total. In one local switch, the initial overlay showed 120/420,
but the run then lasted the full approximately 8.48 seconds and paused at 420.
An initial rendered overlay alone is therefore insufficient evidence of a reset
failure. See the pinned
[mode-control implementation](https://github.com/PCSX2/pcsx2/blob/v2.8.2/pcsx2/Recording/InputRecordingControls.cpp)
and the playback initialization in
[InputRecording.cpp](https://github.com/PCSX2/pcsx2/blob/v2.8.2/pcsx2/Recording/InputRecording.cpp).

## Version 1 wire layout

The fixed header occupies 570 bytes:

| Offset | Size | Value |
| --- | --- | --- |
| 0 | 1 | Format version `1` |
| 1 | 50 | NUL-terminated emulator version |
| 51 | 255 | NUL-terminated author |
| 306 | 255 | NUL-terminated game name |
| 561 | 4 | Total frame count, little-endian |
| 565 | 4 | Rerecord count, initially zero |
| 569 | 1 | Savestate-start flag, `1` |
| 570 | `36 × frames` | Frame rows containing port 0 then port 1 |

Each controller occupies 18 bytes: two active-low button bytes, four analog
coordinates, and twelve pressure channels. Neutral analog coordinates are 127;
unpressed pressure channels are zero. Pressed pressure-sensitive buttons use
255, while Start has only a digital flag. There are no source-game records in
the writer or its tests. Layout and field order were checked against
[InputRecordingFile.h](https://github.com/PCSX2/pcsx2/blob/v2.8.2/pcsx2/Recording/InputRecordingFile.h),
[InputRecordingFile.cpp](https://github.com/PCSX2/pcsx2/blob/v2.8.2/pcsx2/Recording/InputRecordingFile.cpp),
[PadData.h](https://github.com/PCSX2/pcsx2/blob/v2.8.2/pcsx2/Recording/PadData.h), and
[PadData.cpp](https://github.com/PCSX2/pcsx2/blob/v2.8.2/pcsx2/Recording/PadData.cpp).

Synthetic tests check exact header offsets and byte order, all supported digital
and pressure channels, neutral/press/release boundaries, both ports, deterministic
combination ordering, invalid duration/header inputs, filename pairing and
non-overwriting writes. They establish serialization properties, not emulator
frame alignment or game behavior. Each experimental setup needs independent
checks of its starting state, button transitions, stopping point and repeated
outcome; the limited local experiment above does not replace those checks.
