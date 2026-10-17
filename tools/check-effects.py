#!/usr/bin/env python3
"""Enforce ADR 0002: the plugin may not request a non-resident effect archive.

WHY THIS IS A GATE AND NOT A NOTE. `docs/adr/0002-sys-effects-only.md` decided that ported moves
may only use `sys_*` effects, because a fighter-prefixed archive is not resident unless that fighter
is loaded -- so naming one is exactly what creates the CSK Collection dependency the brief forbids.
The ADR then predicts its own failure mode:

    "a future reader will be tempted to 'fix' it by wiring up the real effect names. Doing so
     silently reintroduces the CSK dependency the brief forbids."

That prediction came true in an unattended session on 2026-09-18, which recovered Incineroar's
`gaogaen_revenge_aura` calls verbatim and got as far as checking whether the archive could be loaded
at runtime before re-reading the ADR. It could not -- all 61 `EffectModule__*` entry points request
or cancel effects and not one loads a bank -- so nothing shipped. But "nothing shipped because the
agent happened to re-read the ADR" is not a control, and the failure mode is SILENT: a
fighter-prefixed name compiles, deploys, and simply does nothing on a card where that fighter is
absent. Nothing else in the harness would have caught it.

TWO WRONG VERSIONS WERE WRITTEN FIRST, and both are worth recording because the shape of the test
is the hard part:

  1. An enumerated list of effect macro names. It reached 16 of the 22 names actually present --
     the macros have more spellings than a hand-typed list survives, and a gate that misses a third
     of its target is worse than none.

  2. Any string literal beginning with a fighter archive name, with no regard for call context.
     Four false positives immediately: `chrom_sword`, `cloud_sword` and `palutena_staff` are MESH
     objects in Pikachu's body model, and `gaogaen_swing_throw` is a `LinkModule::send_event_nodes`
     event hash. Fighter-prefixed names are legitimate almost everywhere; it is only in an effect
     request that they are forbidden.

So the test matches effect calls by SHAPE rather than by name -- anything containing `EFFECT`, plus
`EffectModule::*` -- and takes the first `Hash40::new` in each call's own argument window. That
currently matches 12 distinct spellings including `LANDING_EFFECT`, `FOOT_EFFECT` and
`EFFECT_FOLLOW_FLIP_ALPHA`.

AND IT AUDITS ITS OWN COVERAGE, which is what keeps version 1's failure from recurring. Every
`sys_*` literal anywhere in the plugin must be reachable through some call window. If a new macro
spelling appears that the shape pattern does not match, its `sys_*` argument becomes unreachable and
this gate reports that it has gone blind instead of quietly passing.

THE ARCHIVE LIST IS DERIVED, NOT TYPED. `effect/fighter/<name>/` in the arc index gives 94 fighter
archives. `pikachu` is removed: his own bank is resident whenever he is the fighter being played.

Run standalone, or as check.sh gate 4o:  python tools/check-effects.py
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "pikachu-moveset", "plugin", "src")
INDEX = os.path.join(ROOT, "vendor", "arc-file-index.txt.gz")
CACHE = os.path.join(ROOT, "work", "effect-archives.txt")

CALL_RE = re.compile(r"\b(\w*EFFECT\w*|EffectModule::\w+)\s*\(")
NAME_RE = re.compile(r'Hash40::new\("([^"]*)"\)')
SYS_RE = re.compile(r'Hash40::new\("(sys_[a-z0-9_]*)"\)')
# Beyond which no argument of this call can plausibly lie; the next call in the file also bounds it.
WINDOW = 400


def archives():
    """Every fighter effect archive name, minus Pikachu's own. Cached; the index is large."""
    if os.path.exists(CACHE) and os.path.getsize(CACHE) > 0:
        names = open(CACHE, encoding="utf-8").read().split()
    else:
        out = subprocess.run(
            ["bash", "-c", "zcat '%s' | grep -oE '^effect/fighter/[a-z0-9]+/'" % INDEX],
            capture_output=True, text=True,
        ).stdout
        names = sorted({line.split("/")[2] for line in out.splitlines() if line.strip()})
        if names:
            os.makedirs(os.path.dirname(CACHE), exist_ok=True)
            open(CACHE, "w", encoding="utf-8").write("\n".join(names) + "\n")
    return {n for n in names if n != "pikachu"}


def main() -> int:
    banned = archives()
    if not banned:
        print("  !! could not derive the effect archive list from vendor/arc-file-index.txt.gz",
              file=sys.stderr)
        return 1

    requested, spellings, all_sys, bad = {}, set(), set(), []
    for dirpath, _dirs, files in os.walk(SRC):
        for fn in sorted(files):
            if not fn.endswith(".rs"):
                continue
            path = os.path.join(dirpath, fn)
            rel = os.path.relpath(path, ROOT).replace("\\", "/")
            text = open(path, encoding="utf-8").read()
            all_sys |= set(SYS_RE.findall(text))
            for m in CALL_RE.finditer(text):
                spellings.add(m.group(1))
                window = text[m.end():m.end() + WINDOW]
                nxt = CALL_RE.search(window)
                if nxt:
                    window = window[:nxt.start()]
                name = NAME_RE.search(window)
                if not name:
                    continue
                n = name.group(1)
                requested[n] = requested.get(n, 0) + 1
                if n.split("_", 1)[0] in banned:
                    bad.append((n, "%s:%d" % (rel, text.count("\n", 0, m.start()) + 1),
                                m.group(1)))

    # The self-audit: an unreachable sys_ literal means the shape pattern has stopped matching some
    # macro, and this gate is no longer seeing every effect request.
    blind = sorted(all_sys - set(requested))

    for n, where, call in bad:
        print("  FAIL %s requests '%s' at %s" % (call, n, where), file=sys.stderr)
        print("       '%s' is a fighter effect archive and is NOT resident unless that fighter is"
              % n.split("_", 1)[0], file=sys.stderr)
        print("       in the match, so this request silently does nothing. See", file=sys.stderr)
        print("       docs/adr/0002-sys-effects-only.md -- a CSK-requiring variant with full",
              file=sys.stderr)
        print("       source effects is a separate BACKLOG deliverable, not a fix to this one.",
              file=sys.stderr)
    for n in blind:
        print("  FAIL '%s' is never reached through an effect call window" % n, file=sys.stderr)
        print("       A new effect macro spelling is not matched by this gate's shape pattern, so",
              file=sys.stderr)
        print("       it can no longer see every effect request. Widen CALL_RE in",
              file=sys.stderr)
        print("       tools/check-effects.py rather than deleting this check.", file=sys.stderr)

    if bad or blind:
        return 1
    print("  ok  %d effect request(s), %d distinct name(s), all from the resident sys_ archive"
          % (sum(requested.values()), len(requested)))
    print("      %d macro spelling(s) matched; all %d sys_ literals reached, so the gate is not blind"
          % (len(spellings), len(all_sys)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
