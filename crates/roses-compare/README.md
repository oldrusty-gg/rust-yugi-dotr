# Observation and comparison tools

This crate establishes the machinery to collect raw observations and compare
normalized checkpoints. It contains no game rules and no built-in game memory
addresses. Passing synthetic tests demonstrates the harness, not game fidelity.

It also provides a [deterministic PCSX2 input recording generator](RECORDING.md)
for explicit frame segments. That module writes new host files only and requires
a separately supplied companion savestate; it does not control a running game.

## Checkpoint document, schema 1

```json
{
  "schema_version": 1,
  "scenario": "synthetic-counter-example",
  "profile": "synthetic-counter-v1",
  "metadata": {
    "disc_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
    "executable_sha256": "1111111111111111111111111111111111111111111111111111111111111111",
    "observer_version": "example-1",
    "provenance": "synthetic"
  },
  "required_fields": ["/counter"],
  "checkpoints": [
    { "id": "initial", "observed": { "counter": 10 } },
    { "id": "incremented", "observed": { "counter": 11 } }
  ]
}
```

The zero/one hashes above are synthetic placeholders. Real observations must
record the importer-provided hashes of the original disc and executable,
including native runs whose assets came from that original. `profile` identifies
the versioned normalization specification; changing an address mapping, unit, or
field meaning requires changing the profile. Independent observers may have
different `observer_version` values while targeting the same profile.

`compare(&left, &right)` validates metadata, coverage, checkpoint counts and IDs,
then compares checkpoints in sequence and fields in deterministic order. It
returns one of `equal`, `divergent`, or `incomplete`, with the first discrepancy's
checkpoint, JSON pointer and values. The comparison includes **every supplied
field**, not only `required_fields`. Required JSON pointers additionally detect
fields missing from both files. All of the following prevent equality:

- Missing or null observations, including null on both sides.
- An empty observed object, which contains no evidence.
- Empty coverage, invalid JSON pointers or empty checkpoint lists.
- Different scenarios, profiles, coverage sets, source hashes, checkpoint counts
  or ordered checkpoint IDs.

An explicit empty array is known data and can compare equal. JSON types matter:
the string `"10"` differs from the number `10`. Provenance labels distinguish
`original_versus_native`, `original_repeatability`, `native_repeatability` and
`synthetic` evidence. Any synthetic input keeps the report synthetic. Labels and
hashes are supplied by the observer; this format does not authenticate evidence.
Equality covers only the declared observations, never complete game parity.

## Read-only Windows PINE client

```rust,no_run
use roses_compare::pine::{PineClient, DEFAULT_PORT};
use std::time::Duration;

let mut client = PineClient::connect_local(DEFAULT_PORT, Duration::from_secs(2))?;
println!("{:?}", client.identify()?);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The client connects to IPv4 loopback only. It uses the PCSX2 Windows TCP
transport; it does not implement the Unix socket transport. Enable PINE in the
local PCSX2 configuration first. The default slot/port is 28011; callers can
provide another nonzero port and a timeout from greater than zero through 60
seconds. Every request has a total deadline, not merely a timeout reset by each
received byte. A failed or malformed exchange invalidates that connection.

`identify()` returns emulator version/status and, when a VM is active, the title,
serial, game version, and PCSX2 disc CRC. PINE's UUID field is that CRC; it is not
the imported disc's SHA-256 digest.

`read_watches()` takes a caller-provided JSON list of `MemoryWatch` objects. Each
has a unique `name`, decimal `address`, and numeric `width` of 8, 16, 32, or 64 bits.
There are deliberately no example DotR addresses here: no game field mapping has
been established by this crate. Reads are limited to physical 32 MiB EE RAM and
at most 4096 watches per call. Invalid ranges, including overflow and reads that
cross the RAM end, are rejected before issuing a request. Mapped I/O and virtual
aliases are not accepted.

Pause the emulator manually before collecting memory observations. The client
checks paused status before and after reading and rejects captures if either
check differs. A successful `MemoryCapture` still has `atomic: false`: PINE's
service thread does not provide a snapshot lock, and the two status checks
cannot exclude a resume/pause between them. Raw values must be assigned meaning
and verified for repeatability before a separate observer emits normalized
gameplay checkpoints. The client does not control inputs, pause, save, load, or
write emulator memory.

The protocol was checked against the official PCSX2 implementation on
2026-09-27:

- [PINE.cpp](https://github.com/PCSX2/pcsx2/blob/master/pcsx2/PINE.cpp): Windows
  loopback transport, command IDs, framing, result codes and payload layouts.
- [PINE.h](https://github.com/PCSX2/pcsx2/blob/master/pcsx2/PINE.h): default slot.

The implemented subset uses read opcodes 0–3 and metadata opcodes 8 and 11–15.
Frames begin with a four-byte little-endian total length. Replies add a status
byte (`0` success, `255` failure), followed by command payloads in request order.
Strings have a four-byte byte count including one trailing NUL; emulator status
is a four-byte integer (`0` running, `1` paused, `2` shutdown). No upstream source
was copied into the implementation.

## Validation

`cargo test -p roses-compare` exercises exact framing and command bytes through
local fake TCP servers, fragmented replies, all read widths, emulator identity,
paused-before/after checks, explicit failures, bad lengths/status/strings,
truncated replies and deadlines. Comparator tests cover metadata incompatibility,
coverage, missing/unknown fields, JSON pointers, deterministic first differences
and provenance classification. These tests use invented data exclusively.

The project may separately record live PCSX2 probes in ignored local reports;
those observations are distinct from this crate's synthetic protocol tests.

## Observe archived EE memory

`state::capture_state(path, &watches)` applies the same `MemoryWatch` validation
and little-endian reads to a saved RAM buffer without contacting PCSX2. It
returns a serializable `SavedMemoryCapture` with mandatory `state_sha256` and
`ee_memory_sha256`, byte sizes, compression method and raw observations. Game
identity and field semantics are not inferred from the filename or ZIP contents.

The archive is read-only and decoded in memory; no member is extracted to the
filesystem. Limits are 128 MiB of archive bytes, 256 entries, exactly one
`eeMemory.bin`, and exactly 32 MiB of decoded RAM. Stored, Deflate and Zstandard
entries are supported. The local PCSX2 v2.8.2 reference state uses Zstandard
(ZIP method 93), so the reader includes that codec rather than assuming Deflate.

The reader rejects missing, duplicate or incorrectly sized RAM entries,
truncated streams, CRC failures, conflicting local/central metadata, ZIP64,
multi-disk archives, and metadata extras that override names, sizes or encryption.
It checks the raw central directory before invoking `zip` 2.4.2 because that
library collapses duplicate names. Unusual ZIP extensions outside this narrow
format are deliberately unsupported. These checks establish a bounded view of
archived bytes, not the authenticity or emulator compatibility of an arbitrary
file containing `eeMemory.bin`.

`atomic_saved_memory: true` means all requested values came from the same
immutable decoded byte buffer. It does **not** establish whole-machine consistency,
the timing of PCSX2's original state capture, or deterministic replay. Reports
always retain `normalized_game_state: false`.

The byte-to-address interpretation follows the pinned
[PCSX2 v2.8.2 SaveState.cpp](https://github.com/PCSX2/pcsx2/blob/v2.8.2/pcsx2/SaveState.cpp):
the `eeMemory.bin` entry records the main EE RAM array directly. Synthetic tests
exercise full-size compressed RAM, both codecs, width/range boundaries, duplicate
and overriding metadata, source preservation, hashes, corruption and size limits.
