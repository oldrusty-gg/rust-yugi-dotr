"""Audit a friends package before it is zipped.

    python scripts/release/audit.py <package-folder> [--iso <your.iso>] [--samples N] [--with-forbidden-memories]

Fails (exit 1) when the package holds anything but the expected files, when
a file names this PC's user folder or the private research tree, or when
sampled ISO sectors (with --iso) occur inside any file. Prints the DLLs each
exe imports so the run-time requirements can be checked.
"""

import argparse
import os
import random
import struct
import sys
from pathlib import Path

EXES = {"roses-launcher.exe", "roses.exe", "roses-bevy-scene.exe"}
TEXTS = {"README.txt", "KNOWN-ISSUES.txt"}
# Optional: the friends server's address and certificate pin
# (build-release.ps1 -OnlineServer/-OnlinePin), checked below.
OPTIONAL = {"online.json"}
# The launcher's brand and links (scripts/release/platform.json).
TEXTS = TEXTS | {"platform.json"}
# Forbidden Memories as the launcher's second game, only with
# --with-forbidden-memories (build-release.ps1 -ForbiddenMemories <stage>;
# the stage is forbidden2026's scripts/stage-launcher.ps1 output).
FM_FILES = {"fm.exe", "fm-game.exe", "assets/present.wgsl", "games/forbidden-memories.json"}

# Hard failures: absolute paths of the build machine (the repository
# folder, taken from where this script is, so no machine's folder names
# are written here) and the local work tree.
_REPO = Path(__file__).resolve().parents[2]
FORBIDDEN = [
    str(_REPO).encode(),
    str(_REPO).replace("\\", "/").encode(),
    b"\\work\\",
    b":/work/",
]
# Informational: strings worth a look (counts only). The game names the
# optional --research-root layout (relative names such as
# research/story/mesh-scene-simon.json) in its option handling and
# messages; no research file is inside.
REVIEW = [b"research", b"mesh-scene-simon", b"pcsx2", b"SCPH", b"bios", b".iso", b"SLES_515", b"BESLES-51579"]


def user_markers():
    home = Path(os.path.expanduser("~"))
    name = home.name.encode()
    return [
        str(home).encode(),
        str(home).replace("\\", "/").encode(),
        b"\\Users\\" + name,
        b"/Users/" + name,
        b"\\\\Users\\\\" + name,
    ]


def variants(needle):
    yield needle
    yield needle.decode("latin-1").encode("utf-16-le")


def find_all(data, needle, limit=3):
    lower = data.lower()
    hits, start = [], 0
    needle = needle.lower()
    while len(hits) < limit:
        at = lower.find(needle, start)
        if at < 0:
            break
        hits.append(at)
        start = at + 1
    return hits


def context(data, at, width=60):
    chunk = data[max(0, at - width): at + width]
    return "".join(chr(c) if 32 <= c < 127 else "." for c in chunk)


def pe_imports(data):
    """Names of the DLLs in the import and delay-import tables."""
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    sections = struct.unpack_from("<H", data, pe + 6)[0]
    opt = pe + 24
    optional_size = struct.unpack_from("<H", data, pe + 20)[0]
    dirs = opt + 112  # PE32+
    table = []
    for s in range(sections):
        base = opt + optional_size + 40 * s
        vsize, va, raw_size, raw = struct.unpack_from("<IIII", data, base + 8)
        table.append((va, max(vsize, raw_size), raw))

    def offset(rva):
        for va, size, raw in table:
            if va <= rva < va + size:
                return rva - va + raw
        raise ValueError(hex(rva))

    def name(rva):
        o = offset(rva)
        return data[o: data.index(b"\0", o)].decode()

    result = {}
    for kind, index, stride, name_field in (("import", 1, 20, 12), ("delay", 13, 32, 4)):
        rva, size = struct.unpack_from("<II", data, dirs + 8 * index)
        names = []
        if rva:
            o = offset(rva)
            while True:
                entry = data[o: o + stride]
                if entry == b"\0" * stride:
                    break
                names.append(name(struct.unpack_from("<I", entry, name_field)[0]))
                o += stride
        result[kind] = names
    return result


def iso_samples(iso, count, size=64):
    """Random distinctive 64-byte chunks starting at 2048-byte sector starts
    (whole files copied from the disc keep that alignment), plus some at
    random offsets."""
    rng = random.Random(1579)
    total = iso.stat().st_size
    samples = []
    with iso.open("rb") as f:
        attempts = 0
        while len(samples) < count and attempts < count * 50:
            attempts += 1
            if len(samples) % 4 == 3:
                at = rng.randrange(0, total - size)
            else:
                at = rng.randrange(0, total // 2048) * 2048
            f.seek(at)
            chunk = f.read(size)
            if len(set(chunk)) >= 24:  # skip padding / repetitive data
                samples.append((at, chunk))
    return samples


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("package", type=Path)
    parser.add_argument("--iso", type=Path)
    parser.add_argument("--samples", type=int, default=600)
    parser.add_argument("--with-forbidden-memories", action="store_true", help="the package also holds Forbidden Memories (FM_FILES)")
    args = parser.parse_args()
    fm_files = FM_FILES if args.with_forbidden_memories else set()
    root = args.package
    failures = []
    files = sorted(p for p in root.rglob("*") if p.is_file())
    expected_shaders = {
        p.name for p in (Path(__file__).resolve().parents[2] / "crates" / "roses-game" / "assets").glob("*.wgsl")
    }
    print(f"Package {root}: {len(files)} files, {sum(p.stat().st_size for p in files) / 2**20:.1f} MiB")
    for path in files:
        rel = path.relative_to(root).as_posix()
        ok = rel in EXES or rel in TEXTS or rel in OPTIONAL or rel in fm_files or (
            rel.startswith("assets/") and rel.count("/") == 1 and path.name in expected_shaders
        )
        print(f"  {rel:32} {path.stat().st_size:>12,}  {'ok' if ok else 'UNEXPECTED'}")
        if not ok:
            failures.append(f"unexpected file {rel}")
    for name in EXES:
        if not (root / name).is_file():
            failures.append(f"missing {name}")
    for name in TEXTS | fm_files:
        if not (root / name).is_file():
            failures.append(f"missing {name}")
    online = root / "online.json"
    if online.is_file():
        import json
        try:
            config = json.loads(online.read_text(encoding="utf-8-sig"))
            if set(config) - {"server", "name", "tls", "pin"} or not config.get("server"):
                failures.append(f"online.json has unexpected content: {config}")
            else:
                print(f"  online.json: server {config['server']}, pin {config.get('pin', '(public certificate)')}")
        except ValueError as e:
            failures.append(f"online.json is not JSON: {e}")

    forbidden = FORBIDDEN + user_markers()
    blobs = {}
    for path in files:
        data = path.read_bytes()
        blobs[path] = data
        rel = path.relative_to(root).as_posix()
        for needle in forbidden:
            for form in variants(needle):
                for at in find_all(data, form):
                    failures.append(f"{rel}: contains {needle!r} at {at:#x}: {context(data, at)}")
        notes = []
        for needle in REVIEW:
            count = sum(len(find_all(data, form, limit=1000)) for form in variants(needle))
            if count:
                notes.append(f"{needle.decode()}x{count}")
        if notes:
            print(f"  review {rel}: {', '.join(notes)}")
        if path.suffix == ".exe":
            imports = pe_imports(data)
            print(f"  {rel} imports: {', '.join(imports['import'])}")
            if imports["delay"]:
                print(f"  {rel} delay-loads: {', '.join(imports['delay'])}")
            for dll in imports["import"]:
                low = dll.lower()
                if low.startswith(("vcruntime", "msvcp", "api-ms-win-crt", "ucrtbase", "mfplay", "mfplat", "d3dcompiler", "dxcompiler", "dxil")):
                    failures.append(f"{rel}: hard dependency on {dll}")

    if args.iso:
        samples = iso_samples(args.iso, args.samples)
        print(f"ISO check: {len(samples)} distinctive 64-byte samples from {args.iso.name}")
        for path, data in blobs.items():
            hits = [at for at, chunk in samples if chunk in data]
            rel = path.relative_to(root).as_posix()
            print(f"  {rel}: {len(hits)} of {len(samples)} samples found")
            if hits:
                failures.append(f"{rel}: contains ISO bytes (e.g. ISO offset {hits[0]:#x})")
    else:
        print("ISO check skipped (no --iso)")

    if failures:
        print("\nAUDIT FAILED:")
        for failure in failures[:60]:
            print("  " + failure)
        return 1
    print("\nAudit passed: only the expected files; no user paths, research paths or ISO bytes found.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
