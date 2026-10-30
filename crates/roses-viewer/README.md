# Native board snapshot viewer

`roses-viewer` displays an explicit 7-by-7 snapshot using native Windows controls
and original GDI placeholders. Click a tile to select it, then use arrow keys to
change the selection. Selection does not move a unit. The window resizes and the
details panel remains scrollable. There are no movement, summon, turn, combat,
emulator-control or network commands.
Compact tiles show a short card ID when the name will not fit; larger tiles use
a single line with ellipsis. The selected card's full name remains in the details.

The default synthetic demo uses three invented cards and requires no ISO, BIOS,
emulator, imported assets or private files. Optional local names/statistics enrich
captured views. Artwork is not paired with card IDs: that mapping is unverified.

## Build and run

From the workspace root:

```powershell
cargo build -p roses-viewer --release --offline
.\target\release\roses-viewer.exe
.\target\release\roses-viewer.exe --snapshot examples/viewer/demo.json
.\target\release\roses-viewer.exe --capture work/runs/practice-summoned-saved-20260927.json --library work/imports/pal-with-names-20260927
```

The GUI has **Synthetic demo**, **Open capture**, **Open snapshot**, and **Load
local library** buttons. For the library picker, select `manifest.json` in an
existing import. A missing/invalid optional library produces a visible notice;
the current board and raw card IDs remain available. A malformed new snapshot is
rejected while the current valid board is retained.

The board draws raw columns increasing right and rows increasing down. This is
an explicit presentation convention, not a verified original-camera orientation.
`G` and `S` mean raw array group and slot, not player/owner or verified leader
roles. `T` means a numeric terrain code; the palette assigns no terrain rules.
LP, active-side, resource and process values remain labeled candidates. In
particular, the resource bytes are not a verified summoning-points field.

## Platform-neutral data API

The library contains no Win32 types. The Windows renderer is in the executable's
`windows.rs`; a future core need not depend on it or the capture adapter.

```rust
use roses_viewer::{BoardSnapshot, SnapshotDocument};

fn present(document: SnapshotDocument) -> roses_viewer::Result<BoardSnapshot> {
    BoardSnapshot::new(document)
}
```

The `roses-board-view-1` document contains:

- Exactly 49 cells with explicit coordinates, terrain codes and tagged `empty`
  or `occupied { unit_key }` occupancy. Missing data is never a default empty cell.
- Only reciprocally linked board units, with a stable key within the snapshot,
  card ID, coordinates and optional display metadata/raw array location.
- Source provenance and candidate LP values, plus separate unlinked/raw
  diagnostics. Those diagnostics are never drawn as board occupants.

`BoardSnapshot::new` validates and sorts the complete cells/units before exposing
an immutable view through `document()`, `cell()` and `unit()`. `with_library()`
joins existing importer metadata by card ID without changing coordinates or
adding occupants. `from_snapshot_json()` reads the explicit document;
`from_saved_capture()` implements the separate researched PAL observation adapter.

The viewer document is **not a normalized gameplay checkpoint**. Do not feed it
directly into the comparator: names, raw pointers/diagnostics, nullable optional
metadata and differing provenance need a separate, independently researched
projection. A producer can later supply observed native state through the same
validated presentation model without teaching the renderer rules.

Native exports use `ViewOrigin::NativeSimulation` with a required
`OriginalCaptureReference` (capture JSON, archive and EE hashes plus the PAL
address interpretation), bounded `engine_version`, the exact experimental
`roses-private-observed-board-core-1` rules profile, computation `request_sha256`,
and `checkpoint` equal to `before` or `after`. Before is native initialization
imported from that capture; after is computed native state. These references
do not turn the native export into a fresh emulator observation or establish
gameplay parity. The window and `--check` output distinguish all three origins.

Native snapshots must have no per-unit `raw_location`, no unlinked candidates,
and no `raw_candidate_fields`. This prevents original RAM diagnostics from being
carried forward as though the core updated them. Source record mappings belong
in a separate computation trace. All normal board reciprocity checks still apply.

## Saved-capture adapter

Input is the JSON emitted by the existing `roses capture-state` command, with
schema `roses-saved-ee-memory-1`. The adapter reuses `MemoryObservation`,
`MemoryWatch`, width types and bounds from `roses-compare`; it does not parse
savestate archives or talk to PINE itself. `pal_watches()` defines the exact 305
researched names, addresses and widths.

Every required observation must occur exactly once. Wrong names, addresses,
widths, out-of-width values, missing/duplicate observations, unsupported numeric
ranges and inconsistent links are errors. Both input formats are bounded to
1 MiB before parsing. Source archive/EE SHA256 strings and 32 MiB memory metadata
are required, but supplied metadata does not authenticate game identity or
field meanings. The PAL address profile is explicitly an interpretation.

The adapter reads 33 candidate unit records and 49 cells. A cell's nonzero pointer
must target one of those records; its card ID/catalog pointer and coordinates
must agree, and no unit can be referenced twice. Only those linked records become
board occupants. Unlinked valid group-2 cards can share coordinates; they remain
hand/staging candidates, not overlapping units or a third player. Initialized
invalid sentinels are omitted from diagnostics; nondefault unlinked records are
retained. Unknown future captures can be rejected rather than guessed into this
narrow interpretation.

The optional library uses `roses_assets::library::load_library`, preserving its source
metadata consistency checks and bounds. No second asset parser or shared-crate
API changes were needed. Local files are read only; the viewer writes no captures,
libraries or settings. The synthetic demo retains its invented labels even when
a library has been selected for subsequent captured views.

## Verification

```powershell
cargo test -p roses-viewer --offline
cargo +1.92.0 clippy -p roses-viewer --all-targets --offline -- -D warnings
.\target\release\roses-viewer.exe --check --demo
.\target\release\roses-viewer.exe --check --capture work/runs/practice-first-saved-20260927.json
.\target\release\roses-viewer.exe --check --capture work/runs/practice-summoned-saved-20260927.json --library work/imports/pal-with-names-20260927
```

`--check` validates and prints a JSON summary without opening a window. Errors
return exit code 2. An optional missing library is a warning and does not reject
an otherwise valid board. The ignored private acceptance test is:

```powershell
$env:ROSES_VIEWER_CAPTURE_ROOT = (Resolve-Path 'work/runs').Path
cargo test -p roses-viewer private_opening_and_summon_captures_have_two_and_three_occupants -- --ignored
```

At initial implementation handoff, eight synthetic tests and the private acceptance test
passed, as did strict Clippy, the release build and all three headless checks. The private
opening/summon captures produce two/three occupants, and the summon capture keeps
six unlinked candidates out of the board. The coordinating agent confirmed the
initial demo, selection, Right key, saved-capture/library pickers and resizing.
The coordinating agent also opened the final native `after` snapshot and checked
the computed-state label, the moved unit at (3,4), the empty old cell and readable
compact labels. Invalid-file recovery and missing-library notices have automated
coverage but were not exercised interactively. See `IMPLEMENTATION_RESULTS.md` for
the review record and limits.

The native-origin extension adds three regression tests covering preserved
initialization references, invalid/incomplete provenance and stale raw fields.
All 11 ordinary viewer tests, strict Clippy, and debug/release builds pass. The
viewer and CLI release executables were rebuilt after the initial QA window closed.
