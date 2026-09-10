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
done <<< "$out"
n=$(echo "$out" | grep -c "world units")
frames=$(echo "$out" | sed -n 's/.*(n=\([0-9]*\)).*/\1/p' | awk '{s+=$1} END{print s+0}')
[ "$fail" -eq 0 ] && echo "hitbox check: OK ($n moves, $frames hitbox-frames, all within $TOL world units of the source)"
exit "$fail"
