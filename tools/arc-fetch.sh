#!/usr/bin/env bash
# Pull arc paths from the RUNNING game, skipping anything already on disk.
#
#   bash tools/arc-fetch.sh work/missing-nro-paths.txt work/arc-nro
#
# LEAVING THE PLUGIN INSTALLED IS FINE, AND THE CLAIM THAT IT WAS NOT WAS MY ERROR.
#
# This header used to say, at length, that `vendor/libarc_network.nro` hung the training-mode load
# and "costs a playtest", on the strength of reading its 21 lines: a `loop` with no sleep holding a
# second open handle on the 16 GB `rom:/data.arc`. It was a plausible mechanism and it was not
# evidence, and I deleted the plugin off the card on the back of it mid-diagnosis.
#
# It is refuted twice over. `listen()` BLOCKS waiting for a connection, so the loop is one
# accept-and-serve per iteration, not a spin -- and the only way it could spin is the error path,
# which prints `[arc-network] Connection error` and never appeared in a single log. And the plugin
# was dated Sep 10 on the card while pt35 and pt39 both loaded a fighter on Sep 14, so it coexisted
# with roughly forty successful sessions.
#
# The real cause of that hang was `share-to-added` -- see docs/the-weapons.md. Nothing about this
# plugin was involved.
#
# NEVER PROBE PORT 43022 WITH A RAW TCP CONNECT, and this cost a session's bank pull on 2026-10-18.
# The plugin serves ONE connection per accept, so a bare `exec 3<>/dev/tcp/...` that opens and drops
# the socket leaves it serving a dead connection: every later arc-pull then hangs on "connecting..."
# for its whole timeout and arc-probe gets no answer, though the port still reads open. Use
# `arc-probe.exe` to check liveness. If it is already wedged, only a game restart clears it.
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
