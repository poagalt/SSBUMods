#!/usr/bin/env bash
# Every ported ACMD's non-geometric macro arguments must equal the source's, exactly.
#
# Damage, angle, KBG, FKB, BKB, hitlag, SDI, collision attribute, sound level, FT_MOTION_RATE,
# WHOLE_HIT, and every argument of ATTACK_ABS -- which is where the four DK cargo throws keep their
# entire damage and knockback, with no geometry for the hitbox trace to see.
#
# See tools/reach/src/bin/acmdparity.rs for why this is separate from check-hitboxes.sh: geometry
# is supposed to change (topscale scales it), everything else is not.
#
# Driven by tools/moves.tsv, so a move in the trace is a move in this too and there is no second
# list to keep in step.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
P=tools/reach/target/release/acmdparity.exe
D=vendor/SSBU-Dumped-Scripts/smashline
S=pikachu-moveset/plugin/src

fail=0
moves=0
args=0
while read -r f acmd fn stem mod label; do
    [ -z "${f// }" ] && continue
    case "$f" in \#*) continue ;; esac
    src="$D/lua2cpp_$f/$f/$acmd.txt"
    if [ ! -f "$src" ]; then
        echo "FAIL: $label has no dumped source at $src"; fail=1; continue
    fi
    out=$("$P" "$src" "game_$fn" "$S/$mod/mod.rs" "pikachu_game_$fn" 2>&1)
    rc=$?
    moves=$((moves+1))
    n=$(echo "$out" | sed -n 's/.* of \([0-9]*\) non-geometric.*/\1/p')
    args=$((args + ${n:-0}))
    if [ "$rc" != "0" ]; then
        echo "FAIL: $label"
        echo "$out" | sed 's/^/      /'
        fail=1
    fi
done < tools/moves.tsv

if [ "$fail" -eq 0 ]; then
    echo "acmd value check: OK ($moves moves, $args non-geometric args, all identical to the source)"
fi
exit "$fail"
