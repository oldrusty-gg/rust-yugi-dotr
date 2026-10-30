"""Picture cadence of a 50 fps recording (video aid): reads the table
`platform-video --example video-frames <clip> 0 <n> <dir> <every>` prints
(frame, seconds, luma difference from the previous picture) and checks that
every 25 Hz game update holds exactly two pictures: in a moving stretch the
changes fall on one frame parity. Reports the frames where a change lands on
the other parity (an update one picture short or long: a stutter) and the
moving stretches' lengths.

    python -I scripts/video/cadence.py <frames.tsv> [--threshold 0.5] [--out stutter.tsv]
"""

import argparse


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("table")
    ap.add_argument("--threshold", type=float, default=0.5, help="luma difference that counts as a change")
    ap.add_argument("--out")
    a = ap.parse_args()
    rows = []
    for line in open(a.table, encoding="utf-8", errors="replace"):
        parts = line.split()
        if len(parts) >= 3:
            try:
                rows.append((int(parts[0]), float(parts[1]), float(parts[2])))
            except ValueError:
                pass
    changes = [(f, s, d) for f, s, d in rows if d >= a.threshold]
    parity = [0, 0]
    for f, _, _ in changes:
        parity[f & 1] += 1
    main_parity = 0 if parity[0] >= parity[1] else 1
    off = [(f, s, d) for f, s, d in changes if f & 1 != main_parity]
    # Off-parity changes next to an on-parity change of the same update are
    # a two-step picture (both halves move); count those separately from
    # lone ones (a real phase slip).
    changed = {f for f, _, _ in changes}
    lone = [(f, s, d) for f, s, d in off if (f - 1) not in changed and (f + 1) not in changed]
    print("frames %d, changes %d (parity %d: %d, other: %d), off-parity changes next to a change %d, lone %d"
          % (len(rows), len(changes), main_parity, parity[main_parity], parity[1 - main_parity], len(off) - len(lone), len(lone)))
    # Phase slips: the parity of changes switching for good (an update that
    # held 1 or 3 pictures), found as runs where the off parity dominates.
    slips = []
    window = 50
    for i in range(0, len(changes) - window, window // 2):
        chunk = changes[i:i + window]
        offs = sum(1 for f, _, _ in chunk if f & 1 != main_parity)
        if offs > window * 0.6:
            slips.append(chunk[0][1])
    print("stretches of %d changes with the other parity dominant: %d%s" % (window, len(slips), (" at %s s" % ", ".join("%.1f" % s for s in slips[:12])) if slips else ""))
    if a.out:
        with open(a.out, "w", encoding="utf-8") as f:
            for r in off:
                f.write("%d\t%.4f\t%.3f\n" % r)


if __name__ == "__main__":
    main()
