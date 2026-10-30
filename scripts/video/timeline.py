"""The rehearsal's timeline (video aid): minute:second of the duel -> event.

Reads player 1's paced-bot log (ROSES_BOT_LOG: one JSON object per line,
with the duel clock in 25 Hz updates, the revision shown, every side's LP
and what changed on the board, knockouts in MODES matches) and, when
given, the server's replay of the match (the commands by revision: summons
with several hand cards are fusions; a 2-player or an N-player replay).
Writes a Markdown table.

    python -I scripts/video/timeline.py --bot <bot.jsonl> --names <card-names.json>
        [--catalogue <catalogue.json>] [--replay <match replay.json>] --out <timeline.md>
        [--game-err <player 1's game.err> [--fps 50]] [--seats "Jim,mcduck3000,..."]
"""

import argparse
import json
import os


def clock(update):
    s = int(update) // 25
    return "%d:%02d" % (s // 60, s % 60)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bot", required=True)
    ap.add_argument("--names", required=True)
    ap.add_argument("--catalogue")
    ap.add_argument("--replay")
    ap.add_argument("--out", required=True)
    ap.add_argument("--game-err", help="player 1's game.err of a recording: puts the MP4's time beside the duel clock")
    ap.add_argument("--fps", type=float, default=50.0, help="the recording's frame rate (--record-fps; the takes record at 50)")
    ap.add_argument("--seats", help="the seats' names by side, comma separated (MODES: S, N, E, W)")
    a = ap.parse_args()
    names = json.load(open(a.names, encoding="utf-8"))
    seats = [n.strip() for n in a.seats.split(",")] if a.seats else []

    def who(side):
        side = int(side)
        return seats[side] if side < len(seats) else "player %d" % (side + 1)

    def lp_text(lp):
        return "LP " + " / ".join(str(x) for x in lp)

    catalogue = a.catalogue or os.path.join(os.path.dirname(a.names), "catalogue.json")
    fusion_results = set()
    if os.path.exists(catalogue):
        for f in json.load(open(catalogue, encoding="utf-8")).get("fusions", []):
            fusion_results.add(f[2])
    rows = []
    rev_update = []
    if os.path.exists(a.bot):
        for line in open(a.bot, encoding="utf-8"):
            try:
                e = json.loads(line)
            except ValueError:
                continue
            if "revision" in e:
                rev_update.append((e["revision"], e["update"]))
            for n in e.get("notes", []):
                if n.startswith("round"):
                    continue
                tag = ""
                for fid in fusion_results:
                    if fid < len(names) and n.endswith("new unit " + names[fid]):
                        tag = " (fusion result)"
                        break
                rows.append((e["update"], lp_text(e["lp"]), n + tag))
            if e.get("notes") and any(n.startswith("round") for n in e["notes"]):
                rows.append((e["update"], lp_text(e["lp"]), "round %d, %s to act" % (e["round"], who(e["active"]))))
            if "plan_error" in e:
                rows.append((e["update"], "", "bot: " + e["plan_error"]))
    if a.replay and os.path.exists(a.replay):
        rep = json.load(open(a.replay, encoding="utf-8"))
        rev_update.sort()

        def at(rev):
            best = None
            for r, u in rev_update:
                if r >= rev:
                    return u
                best = u
            return best or 0

        for entry in rep.get("entries", []):
            act = entry.get("action", {})
            cmd = act.get("command")
            if isinstance(cmd, dict) and cmd.get("kind") == "choose_summon":
                hand = cmd.get("hand")
                if hand and len(hand) > 1:
                    rows.append((at(entry["revision"]), "", "%s summons from %d hand cards (fusion / power-up chain)" % (who(act.get("side", 0)), len(hand))))
    rows.sort(key=lambda r: r[0])
    # The recording's frame (--fps) when the duel scene began: the last
    # recorder line before the switch to the duel.
    offset = None
    if a.game_err and os.path.exists(a.game_err):
        frame = 0
        for line in open(a.game_err, encoding="utf-8", errors="replace"):
            if line.startswith("Record: frame "):
                try:
                    frame = int(line.split()[2].rstrip(":"))
                except ValueError:
                    pass
            if "Game: Online -> Duel" in line:
                offset = frame / a.fps
                break
    lp_head = " / ".join(seats) if seats else "P1 / P2"
    with open(a.out, "w", encoding="utf-8") as f:
        f.write("# Rehearsal timeline (duel clock, player 1's view)\n\n")
        if offset is not None:
            f.write("The duel begins at %s of the recording (player1.mp4).\n\n" % clock(offset * 25))
            f.write("| Duel | MP4 | LP (%s) | Event |\n|---|---|---|---|\n" % lp_head)
        else:
            f.write("| Time | LP (%s) | Event |\n|---|---|---|\n" % lp_head)
        for u, lp, text in rows:
            for k in range(3, -1, -1):
                text = text.replace("side %d" % k, who(k) if seats else "P%d" % (k + 1))
            if offset is not None:
                f.write("| %s | %s | %s | %s |\n" % (clock(u), clock(u + offset * 25), lp, text))
            else:
                f.write("| %s | %s | %s |\n" % (clock(u), lp, text))
    print("wrote", a.out, len(rows), "rows")


if __name__ == "__main__":
    main()
