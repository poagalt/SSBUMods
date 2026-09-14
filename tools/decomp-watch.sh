#!/usr/bin/env bash
# Is a decomp-all run alive, stalled, or finished? Prints one verdict and exits.
#
#   bash tools/decomp-watch.sh common
#
# A LONG RUN THAT DIES QUIETLY LOOKS EXACTLY LIKE ONE THAT FINISHED, and this project has paid
# for that twice: a deploy chained under a timeout hung for six hours before anyone noticed, and
# a build step that grepped for "^error" against colour-coded output printed nothing through five
# type errors. So the verdict here is derived from the HEARTBEAT, not from whether a process
# exists -- a wedged Ghidra is still a process.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
MOD="${1:?usage: decomp-watch.sh <module>}"
STALL="${2:-600}"          # seconds without a new heartbeat before calling it stalled
PROG="work/decomp/$MOD.progress"

[ -s "$PROG" ] || { echo "$MOD: no progress file -- the run never started"; exit 2; }

last=$(tail -1 "$PROG")
if printf '%s' "$last" | grep -q "DONE"; then
    echo "$MOD: FINISHED -- $last"
    exit 0
fi

# The heartbeat's own UTC stamp, against now. Reading the file's mtime instead would be fooled by
# anything that touches it.
ts=$(printf '%s' "$last" | sed -n 's/^\[\([0-9T:-]*\)Z\].*/\1/p')
age=$(( $(date -u +%s) - $(date -u -d "${ts}Z" +%s 2>/dev/null || echo 0) ))
files=$(ls "vendor/decomp/$MOD" 2>/dev/null | wc -l)
left=$(wc -l < "work/decomp/$MOD.todo" 2>/dev/null || echo "?")
alive=$(ps -W 2>/dev/null | grep -c "[j]ava" || true)

printf '%s: %s file(s) written, %s left, %s java process(es), last heartbeat %ss ago\n' \
    "$MOD" "$files" "$left" "$alive" "$age"
printf '  %s\n' "$last"
if [ "$age" -gt "$STALL" ]; then
    echo "  STALLED -- no heartbeat in over ${STALL}s. Re-run tools/decomp-all.sh $MOD; it resumes."
    exit 1
fi
echo "  alive"
