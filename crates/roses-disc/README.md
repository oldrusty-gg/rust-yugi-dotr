# roses-disc

Read-only inspection of a user-supplied, uncompressed ISO9660 image. It does not
mount the image, extract content to disk, or contain game assets.

```rust,no_run
use roses_disc::Disc;

let mut disc = Disc::open("my-private-disc.iso")?;
for entry in disc.entries() {
    println!("{}: {} bytes", entry.path, entry.size);
}
let configuration = disc.read_file("SYSTEM.CNF", 16 * 1024)?;
let report = disc.inspect()?; // Reads the whole image to calculate SHA-256.
# Ok::<(), roses_disc::Error>(())
```

`Disc::open` validates the primary ISO9660 volume and walks its directory tree.
`entries()` includes directories and files but omits the root and `.`/`..`
aliases. Paths use `/`, omit the leading slash and ISO `;1` suffix, and lookups
are ASCII case-insensitive. `read_file` rejects requests over the caller's byte
limit before allocation. All reads stay inside the declared volume and actual
image. The source must remain unchanged while it is open.

`inspect()` returns a serializable `DiscReport` containing the complete image
hash, inventory, parsed boot metadata, executable hash, and identification flags.
No file payloads are included in the report. Only a full match of the known PAL
image hash, executable hash, and boot metadata yields `known_retail_candidate`.
That status makes no assertion about gameplay correctness or ownership.

Errors distinguish I/O failures, malformed structures, deliberately unsupported
features, missing files, and resource limits. Plain 2048-byte sectors and one
primary volume are required. Joliet/Rock Ridge alternate names are not used;
multi-extent files, interleaving, associated files, extended attribute records,
and multi-volume images fail explicitly. ISO versions other than `;1` are not
accepted. Directory reading is capped at 64 MiB per directory, 256 MiB total,
100,000 entries, and 128 levels. Directory cycles and ambiguous names fail.

Synthetic tests contain no original game bytes:

```text
cargo test -p roses-disc
```

The optional ignored integration test checks the user's private PAL ISO without
writing extracted content. Set `ROSES_TEST_ISO_PATH`, then run:

```text
cargo test -p roses-disc local_pal_iso_matches_known_candidate -- --ignored
```
