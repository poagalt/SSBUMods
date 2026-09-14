#!/usr/bin/env bash
# Pull arc paths from the RUNNING game, skipping anything already on disk.
#
#   bash tools/arc-fetch.sh work/missing-nro-paths.txt work/arc-nro
#
# THE GAME HAS TO BE RUNNING. Port 43022 is served by the arc-network plugin INSIDE the game
# process (vendor/libarc_network.nro), so closing or restarting Smash drops the connection. That
# makes this the one tool here that competes with playtesting for the console, which is why it
# resumes: a pull interrupted by a restart costs only what it had not fetched yet.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
LIST="${1:?usage: arc-fetch.sh <path-list> <outdir> [addr]}"
OUT="${2:?usage: arc-fetch.sh <path-list> <outdir> [addr]}"
ADDR="${3:-<console-ip>:43022}"
mkdir -p "$OUT"

# `prebuilt:/nro/release/x.nro` lands at `<out>/prebuilt/nro/release/x.nro` -- arc-pull strips the
# colon because Windows rejects it. Resume has to strip it the same way or nothing ever looks
# fetched.
TODO=$(mktemp)
while read -r p; do
    case "${p:-}" in '' | \#*) continue ;; esac
    [ -s "$OUT/${p//:/}" ] || printf '%s\n' "$p" >> "$TODO"
done < "$LIST"
n=$(wc -l < "$TODO")
total=$(grep -cvE '^\s*(#|$)' "$LIST")
echo "[$(date -u +%H:%M:%SZ)] $((total - n))/$total already on disk, fetching $n from $ADDR"
[ "$n" -eq 0 ] && { echo "nothing to do"; rm -f "$TODO"; exit 0; }

tools/arc-pull/target/release/arc-pull.exe "$ADDR" "$OUT" < "$TODO"
rc=$?
got=$(find "$OUT" -type f 2>/dev/null | wc -l)
echo "[$(date -u +%H:%M:%SZ)] arc-pull exited $rc; $got file(s) in $OUT"
rm -f "$TODO"
exit "$rc"
