#!/usr/bin/env bash
# Parity harness: every bone a ported hitbox anchors to must exist on Pikachu.
#
# A hitbox on a bone the skeleton lacks is dead weight -- it cannot be positioned, so the move
# silently loses a hit. This is the check that makes tools/bone-remap.sed unnecessary: rather than
# quietly rewriting an unknown bone to `haver` (which lost Sword1's rotation and Stick's offset),
# an unknown bone now fails the build and gets grafted on properly by tools/add-bones.sh.
#
# Two earlier versions of this check were wrong in ways worth not repeating:
#   - It read every `Hash40::new("...")` in the generated source and subtracted a hand-maintained
#     denylist. That swept in effect names, collision attributes and article motion-part names --
#     `to_open`/`to_close` are Mythra's sword-article parts, not bones -- so it reported failures
#     that were not real and needed the denylist grown for each new port.
#   - It matched against `strings` output of the skeleton file, which misses names and cannot
#     distinguish a bone from any other string in the binary.
# Now it takes only the bone argument of ATTACK (positional, so it cannot be confused with
# anything else) and compares against an authoritative ssbh dump of the SHIPPED skeleton.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SKEL="$ROOT/pikachu-moveset/mod/fighter/pikachu/model/body/c00/model.nusktb"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
W="$ROOT/work/bonecheck"
mkdir -p "$W"

[ -f "$SKEL" ] || { echo "FAIL: no shipped skeleton at $SKEL"; exit 1; }
"$T" "$SKEL" "$W/skel.json" >/dev/null 2>&1 || { echo "FAIL: shipped skeleton does not parse"; exit 1; }
jq -r '.bones[].name' "$W/skel.json" | tr -d '\r' | tr 'A-Z' 'a-z' | sort -u > "$W/have.txt"

# `top` is the fighter's origin, not a skeleton bone, so it is never in the nusktb and is always
# valid. Pikachu's own scripts use it 287 times.
echo "top" >> "$W/have.txt"
sort -u -o "$W/have.txt" "$W/have.txt"

# The 4th argument of ATTACK is the bone. Taking it positionally is what makes this precise.
grep -rhoE 'macros::ATTACK\(agent, *[0-9]+, *[0-9]+, *Hash40::new\("[a-z0-9_]+"\)' \
    "$ROOT"/pikachu-moveset/plugin/src/*/mod.rs \
  | grep -oE 'Hash40::new\("[a-z0-9_]+"\)$' | sed 's/Hash40::new("//; s/")//' \
  | sort -u > "$W/need.txt"

fail=0
while read -r b; do
    [ -z "$b" ] && continue
    grep -qx "$b" "$W/have.txt" && continue
    echo "FAIL: hitbox bone '$b' does not exist on Pikachu -- add it in tools/add-bones.sh"
    fail=1
done < "$W/need.txt"

if [ "$fail" -eq 0 ]; then
    echo "bone check: OK ($(wc -l < "$W/need.txt") distinct hitbox bones, all present in a $(($(wc -l < "$W/have.txt") - 1))-bone skeleton)"
fi
exit "$fail"
