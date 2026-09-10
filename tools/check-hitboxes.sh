#!/usr/bin/env bash
# Parity harness: every ported hitbox must land where the source character's does.
#
# Gates on tools/trace-all.sh, which evaluates the shipped animation, the grafted skeleton and our
# own generated ACMD against the source's skeleton and animation, frame by frame. A move that
# cannot be measured (missing FK skeleton) fails too -- unverified is not the same as correct.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOL=0.05
out=$(bash "$ROOT/tools/trace-all.sh" 2>&1 | grep -v "^Parse:")
fail=0
open_radii=""
while IFS= read -r line; do
    [ -z "$line" ] && continue
    name=$(echo "$line" | awk '{print $1}')
    if echo "$line" | grep -qE "no FK skeleton|no FK data|no scale for"; then
        # UNVERIFIED IS NOT CORRECT. trace-all prints "no FK data" when a skeleton or animation was
        # never dumped, and this only matched "no FK skeleton" -- so a move added to moves.tsv
        # without its FK inputs quietly stopped being measured instead of failing. tools/gen-fk.sh
        # generates them all now, and this catches the case where one is still absent.
        echo "FAIL: $name is unmeasurable -- $(echo "$line" | sed "s/.*-- //")"; fail=1; continue
    fi
    if echo "$line" | grep -q "nothing to measure"; then
        continue
    fi
    if echo "$line" | grep -q "NOT MEASURED"; then
        echo "FAIL: $name $(echo "$line" | sed 's/.*NOT MEASURED/NOT MEASURED/')"; fail=1; continue
    fi
    worst=$(echo "$line" | sed -n 's/.*worst \([0-9.]*\) world units.*/\1/p')
    [ -z "$worst" ] && { echo "FAIL: $name produced no measurement"; fail=1; continue; }
    if awk -v w="$worst" -v t="$TOL" 'BEGIN{exit !(w > t)}'; then
        echo "FAIL: $name is $worst world units off the source character (tolerance $TOL)"; fail=1
    fi

    # RADIUS IS GATED SEPARATELY, and only where the answer is known.
    #
    # A hitbox can sit exactly where the source's does and still cover less ground, and only
    # position was ever checked. topscale corrects `top`-anchored radii, where both candidate
    # engine behaviours give the same arithmetic, so those must come out exact. BONE-anchored radii
    # are the open question (topscale's SCALE_BONE_RADII) -- counted and named here rather than
    # failed, because failing a deliberate open question just teaches you to ignore the word FAIL.
    rworst=$(echo "$line" | sed -n 's/.*worst radius \([0-9.]*\) at.*/\1/p')
    rwhere=$(echo "$line" | sed -n 's/.*worst radius [0-9.]* at \(.*\)$/\1/p')
    rbone=${rwhere##* }
    if [ -n "$rworst" ] && awk -v w="$rworst" -v t="$TOL" 'BEGIN{exit !(w > t)}'; then
        if [ "$rbone" = "top" ]; then
            echo "FAIL: $name has a top-anchored radius $rworst off the source at $rwhere"; fail=1
        else
            open_radii="$open_radii $name:$rworst@$rbone"
        fi
    fi
done <<< "$out"
# EVERY ROW IN moves.tsv MUST HAVE PRODUCED A LINE.
#
# The counts here are derived from the output, so a row that produced nothing at all -- a crash, an
# exit code nobody looked at, a name hbtrace rejected -- would simply not appear and the totals
# would shrink without a word. Comparing against the table is one line and makes the trace's
# coverage a checked fact rather than a number to eyeball.
rows=$(grep -cvE '^\s*#|^\s*$' "$ROOT/tools/moves.tsv")
lines=$(echo "$out" | grep -c .)
if [ "$rows" != "$lines" ]; then
    echo "FAIL: moves.tsv has $rows move(s) but the trace produced $lines line(s) -- one traced nothing"
    fail=1
fi
n=$(echo "$out" | grep -c "world units")
frames=$(echo "$out" | sed -n 's/.*(n=\([0-9]*\)).*/\1/p' | awk '{s+=$1} END{print s+0}')
if [ "$fail" -eq 0 ]; then
    echo "hitbox check: OK ($n moves, $frames hitbox-frames, all within $TOL world units of the source)"
    if [ -n "$open_radii" ]; then
        # Named, not hidden. These are the only geometry in the moveset that is not exact, they are
        # all bone-anchored radii, and which way to correct them is one controller-observation away
        # -- see SCALE_BONE_RADII in tools/reach/src/bin/topscale.rs.
        nopen=$(echo "$open_radii" | tr ' ' '\n' | grep -c :)
        echo "  open: $nopen move(s) with a bone-anchored radius off the source:"
        for r in $open_radii; do
            echo "          ${r%%:*} ${r#*:}"
        done
    else
        echo "  ok  every radius matches too, bone-anchored included"
    fi
fi
exit "$fail"
