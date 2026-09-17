#!/usr/bin/env bash
# REFUSES A SHIPPED ANIMATION THAT DRIVES A BONE PIKACHU DOES NOT HAVE.
#
# `anim-dropin.sh` trims every Transform track whose target is absent from the shipped skeleton.
# This asserts the result, across all 74, rather than trusting that the trim ran.
#
# WHY IT EXISTS. The trim used to be bypassed. `anim-dropin.sh` had a fast path -- if the source
# fighter's `scale` param equals Pikachu's, there is no scale compensation to apply, so `cp` the
# animation across verbatim and skip the JSON round trip. That was correct when scaling was the
# only thing the script did, and it silently stopped being correct when the trim was added.
#
# Chrom's scale is 0.9600 and Pikachu's is 0.9600. He was the only source fighter in the moveset
# sitting exactly on that fast path, so his jab shipped with all 96 of his own Transform tracks
# against an 88-bone skeleton -- 51 of them naming bones Pikachu does not have. Ganon's ledge
# attack, also 0.9600, was the only other one. Both went unnoticed through many builds because
# nothing asserted the trim's OUTPUT; the build printed "0 trimmed of foreign tracks" and that
# was read as "nothing needed trimming".
#
# So the gate checks the shipped bytes. A fast path, a caching bug or a step that rewrites an
# animation later all fail it the same way.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
SKEL="$ROOT/pikachu-moveset/mod/fighter/pikachu/model/body/c00/model.nusktb"
DST="$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/c00"
W="$ROOT/work/checkanim"
mkdir -p "$W"
rc=0

if [ ! -f "$SKEL" ]; then
    echo "  no shipped skeleton at $SKEL" >&2
    exit 1
fi
"$T" "$SKEL" "$W/skel.json" >/dev/null 2>&1 || { echo "  cannot parse $SKEL" >&2; exit 1; }
# `tr -d '\r'` on every jq output. Windows jq emits CRLF, command substitution strips the \n and
# not the \r, and a stray CR inside a bone name makes `comm` report every single track foreign.
# That trap has cost this project two false diagnoses; it is not hypothetical.
jq -r '.bones[].name | ascii_downcase' "$W/skel.json" | tr -d '\r' | sort -u > "$W/bones.txt"
nb=$(wc -l < "$W/bones.txt")

n=0; bad=0
for f in "$DST"/*.nuanmb; do
    [ -f "$f" ] || continue
    stem="$(basename "$f" .nuanmb)"
    n=$((n+1))
    if ! "$T" "$f" "$W/a.json" >/dev/null 2>&1; then
        echo "  CANNOT PARSE  $stem" >&2
        rc=1; bad=$((bad+1)); continue
    fi
    jq -r '.groups[]? | select(.group_type=="Transform") | .nodes[]? | .name | ascii_downcase' \
        "$W/a.json" | tr -d '\r' | sort -u > "$W/tr.txt"
    foreign="$(comm -23 "$W/tr.txt" "$W/bones.txt" | tr '\n' ' ')"
    if [ -n "${foreign// }" ]; then
        cnt=$(comm -23 "$W/tr.txt" "$W/bones.txt" | wc -l)
        echo "  FOREIGN TRACKS  $stem drives $cnt bone(s) the skeleton lacks: ${foreign:0:120}" >&2
        rc=1; bad=$((bad+1))
    fi
done

if [ "$rc" -eq 0 ]; then
    echo "  ok  $n shipped animations, every Transform track targets one of Pikachu's $nb bones"
else
    echo "  $bad of $n shipped animations drive a bone that does not exist" >&2
fi
exit $rc
