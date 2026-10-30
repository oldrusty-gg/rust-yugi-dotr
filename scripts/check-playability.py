#!/usr/bin/env python3
"""Capture the actual Bevy executable; keep all game-derived output private.

This is a rendered smoke gate, not original-game parity or a human play test.
Uses Python's standard library only. See check-playability.md for scope.
"""
from __future__ import annotations

import argparse
import binascii
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import struct
import subprocess
import sys
import time
import zlib


ROOT = Path(__file__).resolve().parents[1]
CHECKS = (
    "title", "history", "simon", "name", "starter-deck", "seto",
    "side-choice", "map-arrival", "map-caption", "brest", "map-departure",
    "connected-map", "connected-duel",
)
# Map updates of the connected voyage (campaign_map::sail, frame-matched to
# the original): event 100 sets sail after 48 updates and reaches C2 = 11020
# after 265; event 101 sets sail after 48, 102 starts at 323 and C2 = 0 at
# 372, Chester has risen at 464 (the 620-y31 log's frames).
VOYAGE_SAILING = 120      # event 100: the ship halfway, letterbox closed in
VOYAGE_CAPTION = 200      # event 100: caption "Brest" shown, Brest rising
DEPARTURE_SAILING = 150   # event 101: the ship along the Breton coast
FREE_SETTLED = 100        # event 0: Chester and Tewkesbury risen
MAP_BEFORE_FREE = 265 + 372 + 100
# Raw 512x512 canvas rectangles, deliberately outside the usual top/bottom
# dialogue/HUD bands. Separate left/right terrain checks prevent text or a
# lone cursor from satisfying a scene-presence check. These are smoke limits,
# not target pixels or parameters used by the game's renderer.
REGIONS = {
    "world": [(16, 176, 112, 336), (400, 176, 496, 336)],
    "board": [(24, 192, 232, 360), (280, 192, 488, 360)],
    "map": [(24, 160, 232, 352), (280, 160, 488, 352)],
    "ui": [(32, 96, 480, 416)],
}


def digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def json_write(path: Path, value: object) -> None:
    with path.open("x", encoding="utf-8") as out:
        json.dump(value, out, indent=2)
        out.write("\n")


def paeth(a: int, b: int, c: int) -> int:
    p = a + b - c
    distances = abs(p - a), abs(p - b), abs(p - c)
    return (a, b, c)[distances.index(min(distances))]


def read_capture(path: Path) -> bytes:
    """Read only bounded, non-interlaced 8-bit RGB/RGBA 512px PNG captures."""
    raw = path.read_bytes()
    if len(raw) > 8 * 1024 * 1024 or raw[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("Expected a bounded PNG capture")
    offset, header, compressed, ended = 8, None, bytearray(), False
    while offset + 12 <= len(raw):
        length = struct.unpack_from(">I", raw, offset)[0]
        if offset + length + 12 > len(raw):
            raise ValueError("Truncated PNG chunk")
        kind = raw[offset + 4:offset + 8]
        data = raw[offset + 8:offset + 8 + length]
        crc = struct.unpack_from(">I", raw, offset + 8 + length)[0]
        if binascii.crc32(kind + data) & 0xFFFFFFFF != crc:
            raise ValueError("PNG checksum mismatch")
        if kind == b"IHDR":
            if header is not None or length != 13:
                raise ValueError("Invalid PNG header")
            header = struct.unpack(">IIBBBBB", data)
        elif kind == b"IDAT":
            compressed.extend(data)
        elif kind == b"IEND":
            ended = True
            offset += length + 12
            break
        offset += length + 12
    if not ended or offset != len(raw) or header is None:
        raise ValueError("Incomplete PNG")
    width, height, depth, color, compression, filtering, interlace = header
    if (width, height, depth, compression, filtering, interlace) != (512, 512, 8, 0, 0, 0):
        raise ValueError("Expected non-interlaced 512x512 8-bit PNG")
    if color not in (2, 6):
        raise ValueError("Capture PNG must be RGB or RGBA")
    channels = 3 if color == 2 else 4
    stride = 512 * channels
    expected = 512 * (stride + 1)
    inflater = zlib.decompressobj()
    filtered = inflater.decompress(compressed, expected + 1)
    if len(filtered) != expected or not inflater.eof or inflater.unused_data:
        raise ValueError("Unexpected PNG decompressed size")
    previous, rgb = bytearray(stride), bytearray()
    for y in range(512):
        start = y * (stride + 1)
        mode, row = filtered[start], bytearray(filtered[start + 1:start + stride + 1])
        if mode > 4:
            raise ValueError("Unknown PNG filter")
        for x in range(stride):
            a = row[x - channels] if x >= channels else 0
            b = previous[x]
            c = previous[x - channels] if x >= channels else 0
            predictor = (0, a, b, (a + b) // 2, paeth(a, b, c))[mode]
            row[x] = (row[x] + predictor) & 255
        for x in range(0, stride, channels):
            rgb.extend(row[x:x + 3])
        previous = row
    return bytes(rgb)


def region_stats(rgb: bytes, rect: tuple[int, int, int, int], ui: bool) -> dict:
    x0, y0, x1, y1 = rect
    pixels = [tuple(rgb[(y * 512 + x) * 3:(y * 512 + x) * 3 + 3])
              for y in range(y0, y1) for x in range(x0, x1)]
    brightness = [max(pixel) for pixel in pixels]
    mean = sum(brightness) / len(pixels)
    deviation = math.sqrt(sum((value - mean) ** 2 for value in brightness) / len(pixels))
    lit = sum(value > 8 for value in brightness) / len(pixels)
    colors = len(set(pixels))
    passed = lit >= (0.025 if ui else 0.20) and colors >= 32 and deviation >= 2
    return {
        "rect": rect, "nonblack_fraction": round(lit, 5),
        "unique_rgb": colors, "brightness_stddev": round(deviation, 3),
        "passed": passed,
    }


def inspect_pixels(path: Path, profile: str) -> dict:
    rgb = read_capture(path)
    regions = [region_stats(rgb, rect, profile == "ui") for rect in REGIONS[profile]]
    return {"profile": profile, "regions": regions, "passed": all(r["passed"] for r in regions)}


def checkpoint_plan(game: Path) -> list[dict]:
    game_data = json.loads(game.read_text(encoding="utf-8"))
    story_ref = game_data["story"]
    story = game.parent / story_ref["path"] / story_ref["manifest"]
    stages = {s["id"]: s for s in json.loads(story.read_text(encoding="utf-8"))["stages"]}

    def pages(stage: str) -> list[str]:
        value = stages[stage]
        if value["kind"] != "story" or not value["pages"]:
            raise ValueError(f"Expected nonempty story stage {stage}; update checkpoint path")
        return ["cross"] * len(value["pages"])

    # Name entry: insert the default selected glyph and commit. Then select
    # the explicitly imported skip-practice action and default first deck.
    practice = stages["practice-choice"]
    skip = next(i for i, entry in enumerate(practice["entries"]) if entry["id"] == "skip-practice")
    initial = practice.get("initial_selection", 0)
    direction = "down" if skip >= initial else "up"
    replay = ["cross", "start"] + pages("simon-post-name") + pages("simon-practice-intro")
    replay += [direction] * abs(skip - initial) + ["start"]
    replay += pages("simon-deck-intro") + pages("simon-deck-prompt")
    deck = replay.copy()
    replay += ["start"] + pages("simon-deck-chosen")
    seto = replay.copy()
    replay += pages("seto-arrival") + pages("seto-side-intro")
    side = replay.copy()
    side_menu = stages["side-choice"]
    red = next(i for i, entry in enumerate(side_menu["entries"])
               if entry.get("campaign_side") == "lancastrians")
    delta = red - side_menu.get("initial_selection", 0)
    replay += ["down" if delta >= 0 else "up"] * abs(delta) + ["start"] + pages("seto-leaves")
    # Story 11000 now continues at map event 100 and 11020 at 101: the
    # remaining journey is played by the map (and --story-auto pages 11020).
    if stages["seto-leaves"].get("on_end", {}).get("kind") != "map_event":
        raise ValueError("Story import still ends 11000 in a map stand-in; reimport the story")
    full = replay.copy()

    def opening(name: str, stage: str, profile: str = "world", buttons: list[str] | None = None) -> dict:
        args = ["--start-stage", stage]
        if buttons:
            args += ["--replay", ",".join(buttons)]
        if profile == "world":
            args += ["--capture-live-ticks", "30"]
        return {"name": name, "args": args, "profile": profile, "required_log": []}

    plan = [
        opening("title", "title", "ui"),
        opening("history", "historical-story"),
        opening("simon", "simon-arrival"),
        opening("name", "name-entry", "ui"),
        opening("starter-deck", "name-entry", "ui", deck),
        opening("seto", "name-entry", "world", seto + ["cross"] * min(4, len(pages("seto-arrival")) - 1)),
        opening("side-choice", "name-entry", "ui", side),
    ]
    connected = ["--start-stage", "name-entry", "--replay", ",".join(full), "--story-auto"]
    to_map = "Game: Opening -> Map"
    voyages = (
        ("map-arrival", "map", "map", ["--capture-map-event", "100", "--capture-map-updates", str(VOYAGE_SAILING)],
         [to_map, "map capture gate reached at event 100"]),
        ("map-caption", "map", "map", ["--capture-map-event", "100", "--capture-map-updates", str(VOYAGE_CAPTION)],
         [to_map, "map capture gate reached at event 100"]),
        ("brest", "story", "world", [], [to_map, "Game: Map -> Opening", "fired story stage yugi-at-brest"]),
        ("map-departure", "map", "map", ["--capture-map-event", "101", "--capture-map-updates", str(DEPARTURE_SAILING)],
         [to_map, "Game: Map -> Opening", "Game: Opening -> Map", "map capture gate reached at event 101"]),
        ("connected-map", "map", "map", ["--capture-map-event", "0", "--capture-map-updates", str(FREE_SETTLED)],
         [to_map, "Game: Map -> Opening", "map capture gate reached at event 0"]),
    )
    for name, scene, profile, extra, required in voyages:
        plan.append({"name": name, "args": connected + ["--capture-game-scene", scene] + extra,
                     "profile": profile, "required_log": required})
    # Original first-map cursor begins (256,222), five held Up updates reach
    # Chester; Cross waits for the hover panel to slide in (W+6C = 0x100,
    # 13 updates); page presses separated by release. The map replay is consumed
    # from the voyage's first update on.
    duel_replay = f"none*{MAP_BEFORE_FREE},up*5,none*16,cross,none*80,cross,none*80,cross,none*180"
    plan.append({"name": "connected-duel",
                 "args": connected + ["--capture-game-scene", "duel", "--map-replay", duel_replay],
                 "profile": "board", "required_log": [to_map, "Game: Map -> Duel"]})
    assert [item["name"] for item in plan] == list(CHECKS)
    return plan


def run_check(exe: Path, game: Path, out: Path, item: dict, timeout: int) -> dict:
    name = item["name"]
    png, log = out / f"{name}.png", out / f"{name}.log"
    argv = [str(exe), "--game", str(game), "--research-root", str(ROOT), "--game-seed", "9"]
    argv += item["args"] + ["--capture", str(png)]
    result = {**item, "argv": argv, "log": log.name, "image": png.name, "passed": False}
    json_write(out / f"{name}.command.json", result)
    print(f"Rendering {name}...", flush=True)
    started = time.monotonic()
    try:
        with log.open("xb") as output:
            # Automated runs are silent (ROSES_MUTE keeps sound outputs closed).
            completed = subprocess.run(argv, cwd=ROOT, stdout=output, stderr=subprocess.STDOUT,
                                       timeout=timeout, check=False,
                                       env={**os.environ, "ROSES_MUTE": "1"},
                                       creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        result["exit_code"] = completed.returncode
        text = log.read_text(encoding="utf-8", errors="replace")
        errors = []
        if completed.returncode:
            errors.append(f"Executable exited {completed.returncode}")
        for required in item["required_log"]:
            if required not in text:
                errors.append(f"Missing actual scene transition: {required}")
        if name == "starter-deck" and "[placeholder:" in text:
            errors.append("Starter deck presentation remains incomplete (panel or card stack placeholder)")
        if name in ("map-arrival", "map-caption", "map-departure") and "[stand-in]" in text:
            errors.append("Map interlude still uses a stand-in")
        if png.exists():
            result["image_sha256"] = digest(png)
            result["pixels"] = inspect_pixels(png, item["profile"])
            if not result["pixels"]["passed"]:
                errors.append("Required scene regions are blank, flat, or insufficiently covered")
        else:
            errors.append("No GPU capture produced")
        result["errors"] = errors
        result["passed"] = not errors
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        result["errors"] = [str(error)]
    result["seconds"] = round(time.monotonic() - started, 2)
    if log.exists():
        result["log_sha256"] = digest(log)
    json_write(out / f"{name}.result.json", result)
    print(f"{name}: {'PASS' if result['passed'] else 'FAIL'}", flush=True)
    return result


def self_test() -> None:
    # Failure behavior is the point of this tool: UI-only pixels must not
    # approve a missing scene, and a flat bright fill is not rendered terrain.
    black = bytes(512 * 512 * 3)
    flat = bytes([90, 50, 20]) * (512 * 512)
    text_only = bytearray(black)
    for y in range(24, 96):
        for x in range(30, 480):
            if (x // 3 + y // 3) % 2:
                text_only[(y * 512 + x) * 3:(y * 512 + x) * 3 + 3] = b"\xff\xff\xff"
    scene = bytes(v for y in range(512) for x in range(512)
                  for v in (16 + x % 160, 20 + y % 180, (x + y) % 128))
    for profile in ("map", "board", "world"):
        for rect in REGIONS[profile]:
            assert not region_stats(black, rect, False)["passed"]
            assert not region_stats(flat, rect, False)["passed"]
            assert not region_stats(text_only, rect, False)["passed"]
            assert region_stats(scene, rect, False)["passed"]
    print("Pixel-gate self-test passed: black, flat and UI-only fail; textured scene passes.")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", type=Path, help="Actual executable to verify (never built or copied by this tool)")
    parser.add_argument("--game", type=Path, help="Private user-supplied game.json import")
    parser.add_argument("--out", type=Path, help="New private directory under repository work")
    parser.add_argument("--checks", default=",".join(CHECKS), help="Comma-separated subset; omitted checkpoints remain unverified")
    parser.add_argument("--timeout", type=int, default=100, help="Per-process bound in seconds (1..120)")
    parser.add_argument("--plan-only", action="store_true", help="Validate inputs and print planned checkpoints; launch nothing")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if not args.exe or not args.game or not 1 <= args.timeout <= 120:
        parser.error("--exe, --game and timeout in 1..120 are required")
    exe, game = args.exe.resolve(strict=True), args.game.resolve(strict=True)
    selected = args.checks.split(",")
    if len(set(selected)) != len(selected) or any(name not in CHECKS for name in selected):
        parser.error("--checks must contain distinct known checkpoint names")
    plan = [item for item in checkpoint_plan(game) if item["name"] in selected]
    if args.plan_only:
        print(json.dumps(plan, indent=2))
        return 0
    if args.out is None:
        parser.error("--out is required (a new private directory)")
    out = args.out.resolve()
    if not out.is_relative_to((ROOT / "work").resolve()) or out == ROOT / "work":
        parser.error("Captures contain imported game assets: --out must be under this repo's work directory")
    out.mkdir(parents=True, exist_ok=False)
    metadata = {
        "schema": "roses-rendered-playability-1", "started_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "exe": str(exe), "exe_sha256": digest(exe), "game": str(game), "game_sha256": digest(game),
        "required_checkpoints": CHECKS, "selected": selected,
        "scope": "Actual-executable GPU smoke checks, not original pixel parity or full human play",
        "manual_pending": ["Intro playback and natural completion/skip", "Live keyboard journey and responsive choices",
                           "Tutorial Yes path", "First-duel human input, visible units and result/reward/map return", "Audio"],
    }
    json_write(out / "run.json", metadata)
    results = [run_check(exe, game, out, item, args.timeout) for item in plan]
    missing = [name for name in CHECKS if name not in selected]
    summary = {**metadata, "results": results, "missing_checkpoints": missing,
               "selected_render_checks_passed": all(r["passed"] for r in results),
               "all_required_render_checks_passed": not missing and all(r["passed"] for r in results),
               "full_visible_journey_accepted": False}
    json_write(out / "summary.json", summary)
    print(f"Evidence: {out / 'summary.json'}")
    print("Manual journey acceptance remains pending; GPU checks alone never mark it complete.")
    return 0 if summary["selected_render_checks_passed"] else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, StopIteration) as error:
        print(f"Playability check error: {error}", file=sys.stderr)
        raise SystemExit(2)
