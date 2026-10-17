#!/usr/bin/env python3
"""No ported motion may fall through to a Pikachu expression script that hides or rescales him.

WHAT GATE 3k DOES NOT COVER. A motion entry names four scripts -- `game_`, `effect_`,
`expression_`, `sound_` -- and this port installs the first two and none of the others. 3k catches
the case where a SYNTHESISED motion is left pointing at the template's scripts; that bug made the
up-B invisible for thirty-eight entries, because `special_hi_open` was cloned from a Quick Attack
entry and so named `expression_specialairhi1`, whose body calls
`VisibilityModule::set_whole(false)`.

3k deliberately exempts REPOINTED entries, and it has to: `attack_s4_s` legitimately names
`game_attacks4` because that is what vanilla Pikachu calls it. But the exemption means that every
ported move landing on a motion Pikachu already has still runs HIS `expression_` script, and
nothing checked what those scripts do. Audited on 2026-09-18: 33 of the 77 ported motions resolve
to a real Pikachu `expression_` function and all 33 are clean, so this gate protects a property
that currently holds rather than reporting a defect. The other 44 have no such function and are the
synthesised ones 3k already owns.

WHY IT IS WORTH A GATE ANYWAY. The failure is invisible in code review and spectacular on screen --
a move that is logically correct, measured working, and simply does not draw. It already cost one
playtest. And the risk grows with the move list: the next port onto a repointed motion inherits
whatever Pikachu's script for that motion happens to do, which is not something the porter would
think to look at.

WHAT COUNTS AS DESTRUCTIVE. Calls that can make the fighter or part of him stop drawing, or change
his size: `VisibilityModule::*`, `set_mesh_visibility`, `set_attach_item_visibility` and
`ModelModule::set_scale`. Rumble, flags and sound are not visual and are not checked here -- see
Part 8 of docs/the-article-path.md for what the expression gap costs in full.

Run standalone, or as check.sh gate 3k2:  python tools/check-expressions.py
"""
import glob
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PORTED = os.path.join(ROOT, "tools", "ported-motions.txt")
PIKA = os.path.join(ROOT, "vendor", "SSBU-Dumped-Scripts", "smashline",
                    "lua2cpp_pikachu", "pikachu")

RISK_RE = re.compile(
    r"VisibilityModule::\w+"
    r"|ModelModule::set_mesh_visibility"
    r"|ItemModule::set_attach_item_visibility"
    r"|ModelModule::set_scale"
)


def motions():
    """`<source fighter>:<motion>` per line; the motion is what names the scripts."""
    out = []
    for line in open(PORTED, encoding="utf-8"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        out.append(line.split(":")[-1].strip())
    return out


def main() -> int:
    mot = motions()
    if not mot:
        print("  !! no motions parsed from tools/ported-motions.txt", file=sys.stderr)
        return 1
    if not os.path.isdir(PIKA):
        print("  -   vendor/SSBU-Dumped-Scripts not present; expression fall-through unchecked")
        return 0

    files = {os.path.basename(p)[:-4].lower(): p for p in glob.glob(os.path.join(PIKA, "*.txt"))}
    checked, no_fn, bad = 0, 0, []
    for m in mot:
        stem = m.replace("_", "")
        path = files.get(stem)
        if not path:
            no_fn += 1
            continue
        text = open(path, encoding="utf-8").read()
        fn = re.search(r"fn expression_" + re.escape(stem) + r"\(.*?\n\}", text, re.S)
        if not fn:
            no_fn += 1
            continue
        checked += 1
        hits = sorted(set(RISK_RE.findall(fn.group(0))))
        if hits:
            bad.append((m, stem, hits))

    for m, stem, hits in bad:
        print("  FAIL ported motion '%s' falls through to Pikachu's expression_%s," % (m, stem),
              file=sys.stderr)
        print("       which calls %s." % ", ".join(hits), file=sys.stderr)
        print("       On the ported animation that can hide him or resize him, and the move will",
              file=sys.stderr)
        print("       look broken while its logic measures correct -- this is the bug gate 3k was",
              file=sys.stderr)
        print("       added for, arriving through the repointed path 3k exempts. Install an",
              file=sys.stderr)
        print("       expression_%s of our own, or repoint the entry." % stem, file=sys.stderr)
    if bad:
        return 1

    print("  ok  %d of %d ported motion(s) resolve to a Pikachu expression_ script, none of which"
          % (checked, len(mot)))
    print("      hides or rescales him (%d have none; those are gate 3k's synthesised entries)"
          % no_fn)
    return 0


if __name__ == "__main__":
    sys.exit(main())
