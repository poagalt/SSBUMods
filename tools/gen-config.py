#!/usr/bin/env python3
"""Writes the shipped tree's config.json: the ARCropolis declaration of Rat of Doom's added slot.

WHY THIS REPLACED gen-config.sh. The old generator emitted two keys -- `new-dir-files` and
`share-to-added` -- because the mod replaced Pikachu's own costumes and a replacement needs no
declaring beyond the files it adds. An ADDED slot is a different thing: the directory does not
exist until the config says so, and every file the slot needs has to be either shipped or pointed
at a file that already exists. That is four keys and a few hundred entries of bookkeeping, which is
bad shell and fine Python.

THE SHAPE IS READ OFF TWO WORKING MODS, NOT INVENTED. `Documents/switch/Sans Moveset` (palutena
c120-c127) and `Gogeta Moveset` (minmin c103-c110) are the same tool's output and agree structurally,
which is what makes the rules below derivable rather than guessed:

  new-dir-infos       five per slot: `fighter/X/cN`, and `camera/`, `movie/`, `result/`,
                      `kirbycopy/` beside it. This is what brings the directory into existence.
  new-dir-infos-base  five per slot, and NOTE THE DIFFERENT PATH ORDER -- the infos are
                      `fighter/X/camera/cN` while the bases are keyed `fighter/X/cN/camera`.
                      Transcribed from Sans rather than derived; getting it backwards is the kind
                      of thing that produces a slot the game half-knows about.
  new-dir-files       what each of those directories contains, bucketed by family (below).
  share-to-added      every path the slot needs that the mod does NOT ship, pointed at vanilla c00's
                      copy. This is why Sans's c121-c127 cost almost nothing.

THE BUCKETS, also read off Sans's own `new-dir-files`:

  fighter/X/cN            effect/fighter/X/*, fighter/X/model/*, fighter/X/motion/*,
                          sound/bank/fighter/*, sound/bank/fighter_voice/*
  fighter/X/camera/cN     camera/fighter/X/cN/*
  fighter/X/kirbycopy/cN  fighter/kirby/model/copy_X_cap/cN/*, fighter/kirby/motion/Xbody/cN/*
  fighter/X/movie/cN      empty for Pikachu -- he has no per-costume movie paths at all
  fighter/X/result/cN     likewise

THE ORACLE IS THE ARCHIVE MAP. `work/acquire/arc-paths.txt` is all 590,711 real paths;
`extracted/` is a partial mirror and using it mis-declared 308 paths the last time it was trusted
here (see the note this file inherits from gen-config.sh). A missing map is a hard failure now
rather than a fallback, because an added slot declared from a partial list is a slot with holes in
it, and the symptom is a crash on load rather than a missing animation.

Regenerate with:  python tools/gen-config.py
"""
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SHIP = os.path.join(ROOT, "pikachu-moveset", "ship")
ARCMAP = os.path.join(ROOT, "work", "acquire", "arc-paths.txt")
CHARASLOT = os.path.join(ROOT, "pikachu-moveset", "plugin", "src", "charaslot", "mod.rs")
OUT = os.path.join(SHIP, "config.json")
FIGHTER = "pikachu"


def die(msg):
    sys.stderr.write("  %s\n" % msg)
    sys.exit(1)


# ---- the slot number, from the plugin's own declaration -------------------------------------
# Same parse as tools/relocate-slot.sh, on purpose: if the two ever read it differently, the one
# that is wrong ships a declaration for a directory nothing was moved into.
try:
    src = open(CHARASLOT, encoding="utf-8").read()
except OSError:
    die("cannot read %s to get the slot number" % CHARASLOT)
m = re.search(r"^pub const RAT_COSTUME: usize = (\d+);", src, re.M)
if not m:
    die("no `pub const RAT_COSTUME: usize = N;` in plugin/src/charaslot/mod.rs")
slot = int(m.group(1))
if slot > 127:
    die("RAT_COSTUME is %d; 0-127 only" % slot)
rat = "c%02d" % slot

if not os.path.isfile(ARCMAP) or os.path.getsize(ARCMAP) == 0:
    die("no archive map at work/acquire/arc-paths.txt -- an added slot cannot be declared from the "
        "partial extraction; pull it with tools/arc-fetch.sh")

arc = set(open(ARCMAP, encoding="utf-8", errors="replace").read().split("\n"))
arc.discard("")

# ---- what the slot is made of ----------------------------------------------------------------
# Each entry: (bucket suffix, predicate over a vanilla c00 path). The predicates are prefix tests
# rather than globs so that a path family we have not thought about lands nowhere and is counted,
# instead of being silently swept into the main bucket.
MAIN = ""
BUCKETS = [
    (MAIN, (
        "effect/fighter/%s/" % FIGHTER,
        "fighter/%s/model/" % FIGHTER,
        "fighter/%s/motion/" % FIGHTER,
        "sound/bank/fighter/",
        "sound/bank/fighter_voice/",
    )),
    ("camera", ("camera/fighter/%s/" % FIGHTER,)),
    # BOTH SPELLINGS OF THE COPY MOTION, because the two reference mods do not share one.
    # Palutena's is `fighter/kirby/motion/palutenabody/`; Pikachu's is
    # `fighter/kirby/motion/copy_pikachu_cap/`, which the unbucketed report below found by
    # leaving four of his paths out on the first run rather than sweeping them somewhere wrong.
    ("kirbycopy", (
        "fighter/kirby/model/copy_%s_cap/" % FIGHTER,
        "fighter/kirby/motion/copy_%s_cap/" % FIGHTER,
        "fighter/kirby/motion/%sbody/" % FIGHTER,
    )),
    ("movie", ("fighter/%s/movie/" % FIGHTER,)),
    ("result", ("fighter/%s/result/" % FIGHTER,)),
]

# A costume component is a whole path element (`/c00/`) or the `_c00` suffix the sound banks use.
# The leading `c00` in 23 shipped animation FILENAMES (`c00attack11.nuanmb`) is SSBU's animation
# group naming and is not a costume -- rewriting one breaks the motion_list entry that names it,
# so both patterns below are anchored to avoid it.
DIR_C00 = re.compile(r"/c00/")
SFX_C00 = re.compile(r"_c00(\.[A-Za-z0-9]+)$")


def to_slot(p):
    if DIR_C00.search(p):
        return DIR_C00.sub("/%s/" % rat, p, count=1)
    if SFX_C00.search(p):
        return SFX_C00.sub(r"_%s\1" % rat, p, count=1)
    return None


def bucket_of(p):
    for name, prefixes in BUCKETS:
        if any(p.startswith(x) for x in prefixes):
            return name
    return None


# Every vanilla path that belongs to Pikachu's costume 0, by bucket, as {slot path: vanilla path}.
need = {name: {} for name, _ in BUCKETS}
unbucketed = []
for p in arc:
    if FIGHTER not in p and "/kirby/" not in p:
        continue
    tgt = to_slot(p)
    if tgt is None or tgt == p:
        continue
    b = bucket_of(p)
    if b is None:
        if FIGHTER in p:
            unbucketed.append(p)
        continue
    need[b][tgt] = p

# ---- what we actually ship --------------------------------------------------------------------
if not os.path.isdir(os.path.join(SHIP, "fighter", FIGHTER)):
    die("no shipped tree at %s -- run tools/relocate-slot.sh first" % SHIP)
shipped = set()
for base, _dirs, files in os.walk(SHIP):
    for f in files:
        rel = os.path.relpath(os.path.join(base, f), SHIP).replace(os.sep, "/")
        if rel != "config.json":
            shipped.add(rel)

# A shipped path under the slot that vanilla has no c00 counterpart for is a genuinely NEW file --
# the grafted articles (bow, cshot, fusionsword, parachute) are all of them. It still has to be
# declared, or the directory the game builds will not contain it.
slot_dir = "/%s/" % rat
extra = sorted(p for p in shipped
               if slot_dir in p and p not in need[MAIN] and bucket_of(p.replace(slot_dir, "/c00/")) == MAIN)

# ---- emit ---------------------------------------------------------------------------------------
infos = ["fighter/%s/%s" % (FIGHTER, rat)]
bases = {}
for name, _ in BUCKETS:
    if name:
        infos.append("fighter/%s/%s/%s" % (FIGHTER, name, rat))
# Transcribed from Sans; see the header on the reversed path order.
for sub in ("camera", "cmn"):
    bases["fighter/%s/%s/%s" % (FIGHTER, rat, sub)] = "fighter/%s/c00/%s" % (FIGHTER, sub)
for sub in ("cmn", "bodymotion", "sound"):
    bases["fighter/%s/kirbycopy/%s/%s" % (FIGHTER, rat, sub)] = \
        "fighter/%s/kirbycopy/c00/%s" % (FIGHTER, sub)

new_dir_files = {}
share_to_added = {}
for name, _ in BUCKETS:
    key = "fighter/%s/%s" % (FIGHTER, rat) if not name \
        else "fighter/%s/%s/%s" % (FIGHTER, name, rat)
    paths = sorted(need[name])
    if not name:
        paths = sorted(set(paths) | set(extra))
    new_dir_files[key] = paths
    for tgt, van in need[name].items():
        if tgt not in shipped:
            share_to_added.setdefault(van, []).append(tgt)

# ---- the CSS art, shared rather than shipped ---------------------------------------------------
#
# THE ROW'S PORTRAITS ARE NAMED AFTER THE ROW. `ui/replace/chara/chara_N/chara_N_<name>_<colour>
# .bntx`, where `<name>` is the `name_id` the plugin registers -- "ratofdoom" -- and `<colour>` is
# the index within the row, which for a one-colour row is 00. `original_ui_chara_hash` redirects
# the FIGHTER a row resolves to and not a texture lookup, so without these the CSS draws nothing;
# the first hardware test reported exactly that ("the second one has no character model on the
# character select screen").
#
# SHARED, NOT SHIPPED, AND THAT IS THE WHOLE POINT OF THE MECHANISM. Rat of Doom looks like
# Pikachu, so the right portrait is Pikachu's own -- already in the archive, already on the
# console. `share-to-added` exists to point a path we need at a file that exists, which means no
# bntx has to be pulled, converted or redistributed. Sans ships its own eight because Sans does not
# look like Palutena.
#
# WHICH chara_N EXIST IS ASKED, NOT ASSUMED. Pikachu has chara_0 through chara_7 for colour 00 but
# only some of them for his other colours, and the set differs per fighter -- hardcoding "0..7"
# would declare a path the archive does not have.
NAME_ID = "ratofdoom"
ui_shared = 0
for p in sorted(arc):
    m = re.match(r"^(ui/replace(?:_patch)?/chara/chara_\d+/chara_\d+_)%s_00(\.bntx)$" % FIGHTER, p)
    if m:
        share_to_added.setdefault(p, []).append("%s%s_00%s" % (m.group(1), NAME_ID, m.group(2)))
        ui_shared += 1

# ---- the transplanted effect archive (tools/build-effects.py) ------------------------------------
#
# DECLARED UNDER `fighter/<fighter>/cmn`, not under the slot: smashline's transplant hook loads
# `effect/fighter/<fighter>/transplant/*` for the FIGHTER, whichever costume is loading, and the
# Smash Ultimate Modding Workshop's episode 46 declares its transplant exactly this way
# (`"fighter/chrom/cmn": ["effect/fighter/chrom/transplant/elight/ef_elight.eff"]`).
TRANSPLANT = "effect/fighter/%s/transplant/" % FIGHTER
transplant = sorted(p for p in shipped if p.startswith(TRANSPLANT))
if transplant:
    new_dir_files["fighter/%s/cmn" % FIGHTER] = transplant

for v in share_to_added.values():
    v.sort()

cfg = {
    "new-dir-infos": infos,
    "new-dir-infos-base": bases,
    "new-dir-files": new_dir_files,
    "share-to-added": {k: share_to_added[k] for k in sorted(share_to_added)},
}
with open(OUT, "w", encoding="utf-8", newline="\n") as fh:
    json.dump(cfg, fh, indent=4)
    fh.write("\n")

n_decl = sum(len(v) for v in new_dir_files.values())
n_ship = sum(1 for p in shipped if slot_dir in p)
print("  wrote ship/config.json for slot %s: %d declared path(s) across %d directory/ies, "
      "%d shipped, %d shared from c00"
      % (rat, n_decl, len(new_dir_files), n_ship, len(share_to_added)))
print("    CSS art: %d portrait(s) shared from Pikachu's own as chara_*_%s_00.bntx"
      % (ui_shared, NAME_ID))
print("    effect transplant: %d file(s) declared under fighter/%s/cmn" % (len(transplant), FIGHTER))
if unbucketed:
    print("  NOTE %d pikachu c00 path(s) matched no bucket and were left out:" % len(unbucketed))
    for p in sorted(unbucketed)[:6]:
        print("       %s" % p)
