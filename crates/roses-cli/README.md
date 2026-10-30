# CLI story presentation and movement experiments

`import-opening` now includes the researched historical text/portrait prototype.
To extend an older movie/title import without repeating conversion:

```text
roses import-story --iso game.iso --opening old-opening/manifest.json --out new-opening
roses render-story --manifest new-opening/manifest.json --out new-render-directory
```

The extension verifies the complete supported ISO and existing manifest assets,
copies only referenced assets, imports original message/font data and publishes
the new manifest only after the loader accepts it. Existing directories are never
overwritten. An already-connected New Game destination is rejected. `render-story`
uses the native compositor, records page IDs and RGBA hashes, and requires no GUI.
This validates asset wiring and rendering, not original scene parity. The 3D
backdrop and presentation animation remain incomplete; Simon and gameplay are
not implemented by advancing through these historical paragraphs.

## Movement experiment

```text
roses simulate-move --capture baseline-saved.json --request move.json --out new-directory
```

This command initializes the narrow `roses-core` model from a complete, validated PAL saved-memory capture and computes one explicitly permitted move. It takes no expected endpoint. The capture is bounded to 1 MiB and must contain all 305 researched observations; the request is bounded to 16 KiB. Existing output directories are never overwritten.

Example request (invented fingerprint values and unit identity):

```json
{
  "schema_version": "roses-simulate-move-request-1",
  "scenario": "my isolated movement experiment",
  "profile": "roses-private-observed-board-core-1",
  "metadata": {
    "disc_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "executable_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
  },
  "command": {
    "kind": "move_unit",
    "unit": 2,
    "destination": { "column": 2, "row": 1 }
  },
  "context": {
    "kind": "restricted",
    "eligible_unit": 2,
    "direction": "decreasing_row",
    "permission": "available"
  }
}
```

The caller supplies the context assertion that this operation requires no combat, card effects, terrain interaction or turn logic. The core requires one step into an empty cell with normal terrain at both endpoints. It consumes the supplied model permission; this is not a researched turn counter. Each occupied unit's core ID is its captured array group multiplied by 11 plus its slot. Its raw side byte is imported separately, never inferred from the array group or used to grant authority. The request's disc and executable fingerprints are caller assertions: this command checks their syntax but does not authenticate them against disc data.

Outputs:

- `native-snapshot.json`: comparator checkpoints `before` and `after`. Before is imported model initialization; after is computed. The projection contains board dimensions, occupied units sorted by the string `group:slot` key, occupied cells in row-major order, LP by raw array side, and 49 terrain values. Both checkpoints are explicitly native provenance.
- `trace.json`: exact request hash, source-capture reference, raw unit mapping, initialized core state, computed state/events, and scope limitations.
- `viewer-before.json` and `viewer-after.json`: validated presentation snapshots with native-simulation provenance and honest source references. Original raw diagnostic fields, source record locations and unlinked candidates are omitted from the native views.

The source checkpoint is initialization, not an independently reproduced native game startup. Successful simulation or native replay equality alone proves no original-game behavior. Compare against separately audited reference checkpoints with the same declared projection and scenario. A match covers only those fields and the isolated transition, not input timing, animation, complete memory, effects, turn legality or general gameplay.
