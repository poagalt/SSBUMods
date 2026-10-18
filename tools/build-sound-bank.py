#!/usr/bin/env python3
"""Builds Rat of Doom's own sound bank and drops it into the shipped tree.

    python tools/build-sound-bank.py

Reads tools/sound-slots.tsv, takes Pikachu's c00 bank and the donor banks out of work/sound/arc
(pulled from the console with `bash tools/arc-fetch.sh tools/arc-pull/paths-sound.txt
work/sound/arc`), swaps each listed entry's audio for the donor's, and writes

    ship/sound/bank/fighter/se_pikachu_<slot>.nus3audio    the swapped audio
    ship/sound/bank/fighter/se_pikachu_<slot>.nus3bank     c00's bank, each swapped entry's
                                                            TONE settings taken from the donor

THE TONE GOES WITH THE AUDIO. Every bank entry has a TONE record beside its audio, and the record
carries that sound's own volume (dB) and its SAMPLE COUNT. Swapping only the audio left Pikachu's
records in place, so a donor sound longer than the one it replaced was cut off at Pikachu's length
-- Corrin's u-air (30,449 samples) under a tone that stops at 18,240 was the "sounds bad" the
2026-10-18 playtest heard, and seven of the nineteen swaps were longer than their slot. So each
swapped record's settings are copied from the DONOR's own record, which is the game's value for
exactly that audio. Where the donor's bank was not pulled, only the sample rate and count are set,
from the audio's own IDSP header.

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
import struct
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


# ---- nus3bank TONE records ----------------------------------------------------------------------
# Layout read off se_pikachu_c00.nus3bank (13.0.4): BANKTOC lists the sections; TONE is a u32 count,
# then (offset, size) per record relative to the TONE body, and each record is 12 bytes, a
# length-prefixed NUL-terminated name, padding to 4, then a run of u32 words. The run is NOT a fixed
# layout -- se_pikachu_final02 keeps its rate two words later than the attacks do -- so the sample
# rate and count are FOUND per record: the one place the words read `rate, <channels>, count` for
# that entry's own audio. Every one of Pikachu's records is checked to have exactly one such place
# before anything is written, so a record this does not understand stops the build.


VOLUME = 4


def find_rc(d, w, end, rate, count):
    n = (end - w) // 4
    words = struct.unpack_from("<%dI" % n, d, w)
    hits = [i for i in range(n - 2) if words[i] == rate and words[i + 2] == count]
    return hits[0] if len(hits) == 1 else None


def tone_records(d):
    n = struct.unpack_from("<I", d, 20)[0]
    off = 24 + 8 * n
    secs = {}
    for i in range(n):
        tag = bytes(d[24 + 8 * i:28 + 8 * i])
        size = struct.unpack_from("<I", d, 28 + 8 * i)[0]
        secs[tag] = off
        off += 8 + size
    body = secs[b"TONE"] + 8
    count = struct.unpack_from("<I", d, body)[0]
    out = {}
    for i in range(count):
        o, size = struct.unpack_from("<II", d, body + 4 + 8 * i)
        start = body + o
        nl = d[start + 12]
        name = bytes(d[start + 13:start + 13 + nl - 1]).decode("ascii", "replace").lower()
        words = start + (13 + nl + 3) // 4 * 4
        out[name] = (words, start + size)
    return out


def idsp(blob):
    f = struct.unpack(">8I", blob[4:36])
    return f[2], f[3]


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
plan = []
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
    plan.append((pika.lower(), fighter, donor.lower(), d["data"]))
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
bank = bytearray(open(pika_bank, "rb").read())
ptones = tone_records(bank)
orig_audio = {e["name"].lower(): e["data"] for e in nus3.parse(open(pika_audio, "rb").read())}
# The layout check: each of Pikachu's own records names his own audio's rate and length exactly once.
for name, data in orig_audio.items():
    if name in ptones:
        w, end = ptones[name]
        if find_rc(bank, w, end, *idsp(data)) is None:
            die("TONE layout check failed on %s: no single `rate, ch, count` matching its audio %s"
                % (name, idsp(data)))
full = partial = 0
volume_only = []
donor_tones = {}
for pika, fighter, donor, data in plan:
    w, end = ptones[pika]
    rate, count = idsp(data)
    if fighter not in donor_tones:
        p = bank_path(fighter, "nus3bank")
        donor_tones[fighter] = (open(p, "rb").read(), None) if p else (None, None)
    dbank = donor_tones[fighter][0]
    dt = tone_records(dbank).get(donor) if dbank else None
    if dt and (dt[1] - dt[0]) == (end - w):
        bank[w:end] = dbank[dt[0]:dt[1]]
        full += 1
    else:
        i = find_rc(bank, w, end, *idsp(orig_audio[pika]))
        struct.pack_into("<I", bank, w + 4 * i, rate)
        struct.pack_into("<I", bank, w + 4 * (i + 2), count)
        # THE VOLUME TOO, where the donor's record exists but is a different length: word VOLUME is
        # a float in dB in every record seen (-15.8 .. +3), so that one field is safe to carry
        # across even when the rest of the layout differs. Only if both read as a plausible gain.
        if dt:
            mine = struct.unpack_from("<f", bank, w + 4 * VOLUME)[0]
            theirs = struct.unpack_from("<f", dbank, dt[0] + 4 * VOLUME)[0]
            if -60.0 < mine < 30.0 and -60.0 < theirs < 30.0:
                struct.pack_into("<f", bank, w + 4 * VOLUME, theirs)
                volume_only.append(pika)
        partial += 1
    if find_rc(bank, w, end, rate, count) is None:
        die("%s: the tone does not describe the audio it now carries" % pika)
with open(os.path.join(out_dir, "se_pikachu_%s.nus3bank" % slot), "wb") as fh:
    fh.write(bank)
print("  se_pikachu_%s: %d of %d entries carry donor audio (%d donor banks); tones: %d copied from "
      "the donor's own record, %d with rate/count set from the audio (%d of those also take the "
      "donor's volume)"
      % (slot, len(rows), len(entries), len(donors), full, partial, len(volume_only)))
