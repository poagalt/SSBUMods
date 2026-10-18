#!/usr/bin/env python3
"""Keeps pikachu-moveset/mod/config.json's c00 declarations equal to what the mod tree ships.

WHY THIS IS A BUILD STEP. Gate 3d checks the mod tree's config.json both ways: a shipped file the
archive does not have must be declared, and a declared file must be shipped. Article rows switch
on and off (the rope went on, off and on again on 2026-10-17/18, the rescue came and went with
it), and each switch adds or removes twenty files under model/<article>/c00 and
motion/<article>/c00. Editing the list by hand after every switch was three rounds of the same
gate failure, so the list is now derived: declared = shipped c00 files that are not vanilla paths
(work/acquire/arc-paths.txt, the archive map gate 3d uses too).

Only the `fighter/pikachu/c00` key is touched; everything else is written back as it was read.
"""
import io
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MOD = os.path.join(ROOT, "pikachu-moveset", "mod")
CFG = os.path.join(MOD, "config.json")
ARC = os.path.join(ROOT, "work", "acquire", "arc-paths.txt")
KEY = "fighter/pikachu/c00"

if not os.path.exists(ARC):
    print("  skip  no archive map at %s; config.json left alone" % os.path.relpath(ARC, ROOT))
    sys.exit(0)
van = {l.strip() for l in open(ARC, encoding="utf-8") if l.startswith("fighter/pikachu/")}
shipped = set()
for dp, _dn, fn in os.walk(os.path.join(MOD, "fighter", "pikachu")):
    for f in fn:
        rel = os.path.relpath(os.path.join(dp, f), MOD).replace(os.sep, "/")
        if "/c00/" in rel:
            shipped.add(rel)
want = sorted(shipped - van)
raw = io.open(CFG, encoding="utf-8", newline="").read()
d = json.loads(raw)
have = d["new-dir-files"].get(KEY, [])
if sorted(have) == want:
    print("  config.json: %d c00 declaration(s), already in step with the tree" % len(want))
    sys.exit(0)
gone = sorted(set(have) - set(want))
added = sorted(set(want) - set(have))
d["new-dir-files"][KEY] = want
out = json.dumps(d, indent=4) + "\n"
out = out.replace('"share-to-added": {}', '"share-to-added": {\n\n    }')
io.open(CFG, "w", encoding="utf-8", newline="\n").write(out)
def dirs(paths):
    return sorted({"/".join(p.split("/")[2:4]) for p in paths})
print("  config.json: %d c00 declaration(s); added %d %s, removed %d %s"
      % (len(want), len(added), dirs(added), len(gone), dirs(gone)))
