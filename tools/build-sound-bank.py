#!/usr/bin/env python3
"""Builds Rat of Doom's own sound bank and drops it into the shipped tree.

    python tools/build-sound-bank.py

Reads tools/sound-slots.tsv, takes Pikachu's c00 bank and the donor banks out of work/sound/arc
(pulled from the console with `bash tools/arc-fetch.sh tools/arc-pull/paths-sound.txt
work/sound/arc`), swaps each listed entry's audio for the donor's, and writes

    ship/sound/bank/fighter/se_pikachu_<slot>.nus3audio    the swapped audio
    ship/sound/bank/fighter/se_pikachu_<slot>.nus3bank     c00's bank, byte for byte

THE BANK IS SHIPPED, NOT SHARED, AND THAT IS DELIBERATE. ARCropolis gives a file a mod ADDS a
fresh bank ID (`src/replacement/addition.rs`: `add_file` sets `unshared_nus3bank`, and
`threads.rs` patches the ID at load), while a `share-to-added` file keeps c00's. A c120 bank with
c00's ID beside vanilla Pikachu's in one match is two banks claiming one ID. Gogeta ships its slot
bank the same way. The `.tonelabel` stays shared from c00, because every label is unchanged.

Writes nothing into the repo: ship/ is generated on every build and gitignored, and the donor
audio is Nintendo's, so it is reproduced from the console pull rather than committed.
"""
import importlib.util
import os
import re
import shutil
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ARC = os.path.join(ROOT, "work", "sound", "arc", "sound", "bank", "fighter")
SHIP = os.path.join(ROOT, "pikachu-moveset", "ship")
SLOTS = os.path.join(ROOT, "tools", "sound-slots.tsv")

spec = importlib.util.spec_from_file_location("nus3audio", os.path.join(ROOT, "tools", "nus3audio.py"))
nus3 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(nus3)


def die(msg):
    sys.stderr.write("  REFUSING: %s\n" % msg)
    sys.exit(1)


def bank_path(fighter, ext):
    """The c00 bank if the fighter has per-slot banks, else the base one."""
    for name in ("se_%s_c00.%s" % (fighter, ext), "se_%s.%s" % (fighter, ext)):
        p = os.path.join(ARC, name)
        if os.path.isfile(p):
            return p
    return None


src = open(os.path.join(ROOT, "pikachu-moveset", "plugin", "src", "charaslot", "mod.rs"), encoding="utf-8").read()
m = re.search(r"^pub const RAT_COSTUME: usize = (\d+);", src, re.M)
if not m:
    die("no `pub const RAT_COSTUME: usize = N;` in plugin/src/charaslot/mod.rs")
slot = "c%02d" % int(m.group(1))

pika_audio = bank_path("pikachu", "nus3audio")
pika_bank = bank_path("pikachu", "nus3bank")
if not pika_audio or not pika_bank:
    die("Pikachu's bank is not in work/sound/arc -- pull it with "
        "`bash tools/arc-fetch.sh tools/arc-pull/paths-sound.txt work/sound/arc` while Smash runs")
entries = nus3.parse(open(pika_audio, "rb").read())
index = {e["name"].lower(): i for i, e in enumerate(entries)}

rows = []
for line in open(SLOTS, encoding="utf-8"):
    if line.startswith("#") or not line.strip():
        continue
    parts = line.rstrip("\n").split("\t")
    if len(parts) != 3:
        die("tools/sound-slots.tsv: bad row %r" % line)
    rows.append(parts)

donors = {}
seen = set()
for pika, fighter, donor in rows:
    i = index.get(pika.lower())
    if i is None:
        die("%s is not an entry in Pikachu's bank" % pika)
    if i in seen:
        die("%s is listed twice" % pika)
    seen.add(i)
    if fighter not in donors:
        p = bank_path(fighter, "nus3audio")
        if not p:
            die("no %s bank in work/sound/arc" % fighter)
        donors[fighter] = {e["name"].lower(): e for e in nus3.parse(open(p, "rb").read())}
    d = donors[fighter].get(donor.lower())
    if d is None:
        die("%s is not an entry in %s's bank" % (donor, fighter))
    # A replaced entry gets a key of its own, so if it shared a blob with an untouched entry, the
    # untouched one keeps the original audio. See tools/nus3audio.py's `build`.
    orig = entries[i]["key"]
    entries[i] = dict(entries[i], data=d["data"], key=(orig[0], orig[1], "new", i))

blob = nus3.build(entries)
back = nus3.parse(blob)
for i, (a, b) in enumerate(zip(entries, back)):
    if a["name"] != b["name"] or a["id"] != b["id"] or a["data"] != b["data"]:
        die("round trip failed at entry %d" % i)

out_dir = os.path.join(SHIP, "sound", "bank", "fighter")
if not os.path.isdir(os.path.join(SHIP, "fighter")):
    die("no shipped tree -- run tools/relocate-slot.sh first")
os.makedirs(out_dir, exist_ok=True)
with open(os.path.join(out_dir, "se_pikachu_%s.nus3audio" % slot), "wb") as fh:
    fh.write(blob)
shutil.copyfile(pika_bank, os.path.join(out_dir, "se_pikachu_%s.nus3bank" % slot))
print("  se_pikachu_%s: %d of %d entries carry donor audio (%d donor banks); bank copied from c00"
      % (slot, len(rows), len(entries), len(donors)))
