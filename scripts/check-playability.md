# Rendered playability checks

`check-playability.py` runs the **supplied game executable on the GPU** and
captures the rendered output. It does not build, import, copy, launch a visible
window, alter the play copy, or use screenshots as game inputs. Python 3.11+
standard library is sufficient; no Pillow or external image tools are needed.

```powershell
python scripts/check-playability.py --self-test
python scripts/check-playability.py --exe crates/roses-game/target/debug/roses-bevy-scene.exe --game work/play/roses-data/game-20261001-claude-v7/game.json --out work/research/playability-checks/run-001
```

Use a **new** output directory under this repository's `work` for each run.
Screenshots, logs and replay-derived data are private and must not be shipped.
The runner refuses reuse or output outside `work`, does not download tools,
uses argument arrays without a shell, and bounds each child process. It records
executable/import SHA256, exact arguments, logs, PNG hashes, per-region pixel
statistics, individual results, and `summary.json`. Failed checks preserve
their evidence and do not prevent the remaining checkpoints from running.

`--plan-only` prints the planned checks without launching anything.
`--checks connected-map,connected-duel` runs a targeted subset. A successful
subset leaves all other checkpoints explicitly unverified in the summary.

The twelve checkpoints are title, historical story, Simon, name entry, starter
deck selector, Seto, side choice, first map interlude, Brest, second map
interlude, connected map, and connected first duel. Replays use page counts
from the supplied story manifest. They commit a one-glyph name, skip practice,
select the first starter candidate and choose the Red Rose route. Nothing
embeds the original card/deck/asset tables.

Connected map/duel captures use `--capture-game-scene map|duel`, which requires
`--game` and `--capture`, cannot be combined with early-exit flags, and refuses
wrong scenes or stopped games. Readiness is cleared at every scene change so
despawned opening assets cannot stall or approve a later framebuffer. In
capture mode, `--map-replay` enters the same map step path directly because
there is no native window; directions remain held and Cross is a rising edge.
Normal windowed replay remains unchanged. A map-target capture freezes at the
reached map, while a duel-target capture replays Chester selection through the
real map→duel transition. A duel capture freezes at the first gameplay frame.

Pixel checks are deliberately modest **scene-presence** checks. Map and duel
each need varied, nonblack content in two separated world regions outside
the usual text/HUD bands. Black, flat-color and UI-only synthetic examples
fail the tool's self-test. The checker also rejects explicit starter-selector
and map-interlude placeholder reports. **Any** `[placeholder:` status at the
starter-deck checkpoint fails, including the remaining three-card stack when
the information panel has been imported. A clearer selector is useful progress
but must not silently turn this whole-screen acceptance check green.
It does not compare against a golden
screenshot or guarantee geometry, layout, color, animation, models, rules,
timing, audio, or original-game fidelity. A rendered board with missing
monster models can pass this smoke check; the manual acceptance still fails.

As of this tool's introduction, the old v7 import has known missing deck/map
interlude/Brest presentation. Those checkpoints should fail and remain visible
in reports. Do not lower thresholds or exclude failed stages to call the
whole journey complete.

Before updating a user's play copy, retain this report for the exact candidate
binary and separately verify the actual visible journey: intro natural end
and skip; keyboard navigation and readable choices; both Practice No and Yes;
map navigation; a human duel with visible units; result/rewards/map return;
and audio. Existing replays run the opening model before GPU startup, so they
cannot prove every opening transition was rendered. The tool always leaves
`full_visible_journey_accepted` false; acceptance needs that separate evidence.
