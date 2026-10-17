#!/usr/bin/env bash
# Ships the moveset's animations for EVERY Pikachu costume, not just the default one.
#
# THE HOLE THIS CLOSES. The mod shipped `fighter/pikachu/motion/body/c00` and nothing else, and
# the in-game self-test announced it every load: "the mod ships c00 ONLY -- any other colour has
# no added animations at all". That is not a cosmetic limit. Vanilla Pikachu has eight FULL
# costume directories -- 338 motion files each, nothing shared between them -- so picking any
# Pikachu but the default gave a moveset whose 30 ported animations did not exist and whose 45
# modified Pikachu animations were the vanilla ones. The plugin itself has no costume gate; it
# only REPORTS the colour, so the code was always running and only the assets were missing.
#
# WHAT IS MIRRORED, AND WHAT DELIBERATELY IS NOT.
#
#   motion/body/c00  ->  c01..c07     YES. An animation is costume-independent: same filenames in
#                                    every vanilla costume directory (verified against the archive
#                                    map -- c00 and c05 have identical filename sets), and the
#                                    scale compensation `anim-dropin.sh` applies does not vary by
#                                    colour.
#
#   model/body/c00   ->  c01..c07     NO, and copying it would be a visible REGRESSION. Each
#                                    costume's body model carries its own textures; dropping c00's
#                                    model on c05 would make every Pikachu the default yellow. The
#                                    grafted weapons therefore remain c00-only until the merge is
#                                    taught to read each costume's own model, which needs those
#                                    models pulled first -- `extracted/.../model/body/c05` holds a
#                                    single `model.nusktb`.
#
# So on a non-default costume the moveset now plays every move with the right animation and the
# right colour, and Chrom's jab swings no sword. That is strictly better than no moves at all, and
# the remaining gap is recorded rather than hidden.
#
# `motion_list.bin` IS MIRRORED TOO, AND THAT IS CHECKED RATHER THAN ASSUMED. It is one of the 75
# files, it is the thing that makes the 30 synthesized motions resolvable at all
# (`src/specials/mod.rs`: "they are synthesized into motion_list.bin by ..."), and vanilla has a
# separate copy in all eight costume directories -- so replacing seven of them with c00's version
# would be wrong if they differed. Pulled c00, c01, c04 and c07 out of the archive and compared:
# 43,428 bytes and md5 c9ddb186ce85318e1abaa25832dd0b08, all four identical. They are the same
# file per costume, so shipping ours everywhere is exact.
#
# THE FILENAMES LOOK LIKE COSTUMES AND ARE NOT. `c00attack11.nuanmb`, `c01attacks3s.nuanmb`,
# `b02escapeair.nuanmb` -- the leading letter and two digits are SSBU's animation-GROUP naming,
# which is why 23 of the 75 shipped files carry one and why they are copied verbatim rather than
# renamed per costume. Renaming them would break every one.
#
# Runs from build.sh after anim-dropin.sh, which regenerates c00 on every build, so this has to
# re-mirror every build too. Idempotent.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/c00"

if [ ! -d "$SRC" ]; then
    echo "  no c00 motion directory -- run tools/anim-dropin.sh first" >&2
    exit 1
fi

n_src=$(find "$SRC" -maxdepth 1 -type f | wc -l)
if [ "$n_src" -lt 10 ]; then
    echo "  REFUSING: c00 has only $n_src file(s), which is not a built animation set" >&2
    exit 1
fi

# ONE COPY PER COSTUME, NOT ONE PER FILE, and that is the difference between 67 seconds and
# about five. This used to `cp` each animation individually -- 75 animations into 7 costumes is
# 525 invocations, plus a `[ -e ]` per existing file for the retire pass. Under Git Bash a process
# spawn costs on the order of 100ms, so the spawns WERE the phase: measured at 67s of a 249s
# build, the single largest step in it, ahead of the 25s cargo release compile.
#
# WIPING IS EQUIVALENT TO THE OLD RETIRE PASS, not a shortcut past it. The old loop deleted
# anything in a costume directory that c00 does not carry, so the destination was always a subset
# of c00 by construction -- there is nothing costume-specific in `motion/body/cNN` for this mod to
# preserve. Removing the directory and recreating it reaches the same state in two spawns instead
# of a hundred, and it keeps the property that mattered: a dropped animation cannot linger here
# after it has gone from c00, which is the shipped-but-undeclared trap deploy.sh's retire list
# exists for and which nothing else ever looks in these directories to catch.
for c in c01 c02 c03 c04 c05 c06 c07; do
    dst="$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/$c"
    rm -rf "$dst"
    mkdir -p "$dst"
    cp -r "$SRC/." "$dst/"
done
copied=$(( n_src * 7 ))
echo "  mirrored $n_src animation(s) into 7 costume(s): $copied file(s)"
