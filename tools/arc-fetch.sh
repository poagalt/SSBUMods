#!/usr/bin/env bash
# Pull arc paths from the RUNNING game, skipping anything already on disk.
#
#   bash tools/arc-fetch.sh work/missing-nro-paths.txt work/arc-nro
#
# REMOVE THE PLUGIN WHEN YOU ARE DONE, AND THAT IS NOT HOUSEKEEPING -- IT COSTS A PLAYTEST.
#
# `vendor/libarc_network.nro` is 21 lines and one of them is this:
#
#     std::thread::spawn(|| loop {
#         Networked::new_buffered(File::open("rom:/data.arc").unwrap(), ("0.0.0.0", PORT))
#             .unwrap().listen()
#     });
#
# A second open handle on the whole 16 GB archive, held for the life of the process, re-opened
# every time `listen()` returns, in a loop with no sleep. Left installed after the weapon merge it
# hung the training-mode load INDEFINITELY -- not a crash: the loading animation kept running, no
# crash report was written, ARCropolis finished its filesystem work and went quiet, our plugin
# installed all the way through `articles::install()`, and no fighter object was ever created.
# Twenty-five minutes on the loading screen before it was noticed, then a second boot that did the
# same thing.
#
# So: after a pull, delete it from
# `/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/`. `bash tools/ftpq.sh del <path>`
# does it, and `vendor/libarc_network.nro` is the file to put back when the next pull needs it.
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
