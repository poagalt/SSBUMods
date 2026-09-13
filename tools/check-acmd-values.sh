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
kept=0
drop=0
while read -r f acmd fn stem mod label; do
    [ -z "${f// }" ] && continue
    case "$f" in \#*) continue ;; esac
    src="$D/lua2cpp_$f/$f/$acmd.txt"
    if [ ! -f "$src" ]; then
        echo "FAIL: $label has no dumped source at $src"; fail=1; continue
    fi
    # THE EFFECT SCRIPTS ARE CHECKED TOO, where both sides have one.
    #
    # tools/reach/src/bin/sysfx.rs rewrites them -- it keeps sys_ effects and drops fighter-owned
    # ones per ADR 0002 -- and it currently drops zero, so a surviving effect call should be
    # byte-identical to the source's. Nothing verified that, which meant sysfx could have ALTERED
    # an effect rather than dropping it and no check would notice.
    #
    # Skipped rather than failed when either side lacks the function: a move whose source has no
    # effect script is common, and a move whose effects were all dropped legitimately has none.
    # The acmdparity call itself distinguishes a missing function from an empty one.
    for kind in game effect; do
        if ! grep -q "fn ${kind}_${fn}(" "$src" 2>/dev/null; then continue; fi
        if ! grep -q "fn pikachu_${kind}_${fn}(" "$S/$mod/mod.rs" 2>/dev/null; then continue; fi
        subset=""
        [ "$kind" = "effect" ] && subset="--subset"
        # THE BONE ANCHOR IS EXEMPT FOR THE SCRIPTS `tools/reach rebase` REWROTE, and the list is
        # the one that pass WROTE rather than one declared here. Chrom's jab and Palutena's n-air
        # ride grafted bones that do not exist in the runtime skeleton, so their hitboxes are moved
        # onto `haver` with the offsets rotated; every other argument is still compared.
        reb=""
        if [ -f "$ROOT/work/rebase/rebased.txt" ] \
           && grep -qx "$fn" "$ROOT/work/rebase/rebased.txt" 2>/dev/null; then
            reb="--rebased"
        fi
        out=$("$P" "$src" "${kind}_$fn" "$S/$mod/mod.rs" "pikachu_${kind}_$fn" $subset $reb 2>&1)
        rc=$?
        [ "$kind" = "game" ] && moves=$((moves+1))
        if [ "$kind" = "game" ]; then
            n=$(echo "$out" | sed -n 's/.* of \([0-9]*\) non-geometric.*/\1/p')
            args=$((args + ${n:-0}))
        else
            k=$(echo "$out" | sed -n 's/.*altered, \([0-9]*\) kept.*/\1/p')
            d=$(echo "$out" | sed -n 's/.* \([0-9]*\) dropped of.*/\1/p')
            kept=$((kept + ${k:-0})); drop=$((drop + ${d:-0}))
        fi
        if [ "$rc" != "0" ]; then
            echo "FAIL: $label ($kind)"
            echo "$out" | sed 's/^/      /'
            fail=1
        fi
    done
done < tools/moves.tsv

if [ "$fail" -eq 0 ]; then
    echo "acmd value check: OK ($moves moves, $args non-geometric args, all identical to the source)"
    echo "  effects: $kept call(s) kept and identical, $drop dropped as fighter-owned (ADR 0002)"
fi
exit "$fail"
