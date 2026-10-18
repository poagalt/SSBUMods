#!/usr/bin/env bash
# Pulls the newest Switch screenshots and capture clips off the card, so a look at the game costs
# one button press instead of a paragraph of description.
#
# WHY THIS EXISTS. Every article bug in this project has been reported as a shape -- "in the wrong
# hand", "straight down through the model", "stuck inside his body facing up" -- and prose is a
# lossy channel for three dimensions. A JPG is not: it can be opened and looked at directly. The
# Capture button writes one to /Nintendo/Album/YYYY/MM/DD/ with no menus and no interruption to
# play, so the user's whole cost is pressing it at the moment something looks wrong.
#
# HOLD Capture and the console writes the last 30 seconds as an MP4 instead. That matters for
# anything moving: a still cannot show whether a weapon is tracking the hand or lagging a frame
# behind it. ffmpeg is on this machine, so `--frames` splits the newest clip into stills.
#
# IT RUNS WHILE THE GAME IS OPEN. This was believed to be impossible for most of the project --
# "sys-ftpd exits when a game launches" -- and that belief came from anonymous requests failing
# with `430 Access denied`, an AUTHENTICATION error that looks like a transport one. With the
# credentials deploy.sh has always used, listings and downloads both work with Smash running and
# a match in progress. So the loop is: play, press Capture when something looks wrong, and the
# shots can be pulled immediately without quitting anything.
#
#   bash tools/pull-shots.sh              newest 5 stills
#   bash tools/pull-shots.sh 12           newest 12 stills
#   bash tools/pull-shots.sh 5 --frames   also split the newest clip into stills at 6 fps
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
. "$ROOT/tools/console.sh"
HOST="$CONSOLE_HOST"
PORT="${SWITCH_FTP_PORT:-5000}"
B="ftp://$HOST:$PORT"
N="${1:-5}"
MODE="${2:-}"
OUT="$ROOT/work/shots"

# CREDENTIALS, THE SAME ONES tools/deploy.sh USES. Anonymous access worked for a while and then
# stopped with `430 Access denied`, which reads like a data-connection fault and is not one -- it
# is authentication. deploy.sh has always read this file; these tools never did, so they broke the
# first time the server wanted a login and the error pointed nowhere near the cause.
FTP_USER="${FTP_USER:-poaga}"
FTP_PASS="$(cat "$ROOT/.ftp-credentials.local" 2>/dev/null | tr -d '\r\n')"
CURL_AUTH=()
[ -n "$FTP_PASS" ] && CURL_AUTH=(-u "$FTP_USER:$FTP_PASS")

mkdir -p "$OUT"

if ! curl -s "${CURL_AUTH[@]}" --connect-timeout 5 --max-time 10 -l "$B/" >/dev/null 2>&1; then
    echo "no FTP on $HOST:$PORT."
    echo "Relaunch ftpd on the console (hbmenu -> ftpd)."
    echo "NOTE: the game does NOT have to be closed. That was believed for most of this project"
    echo "and it is wrong -- with the credentials in .ftp-credentials.local the server serves"
    echo "normally while Smash is running. The symptom that caused the confusion was \`430 Access"
    echo "denied\`, which reads like a transport fault and is authentication."
    exit 1
fi

# The album is date-nested, so the newest day has to be walked to rather than globbed. Three
# listings (year, month, day) beats pulling an index of the whole card.
# BASENAME, BECAUSE sys-ftpd RETURNS FULL PATHS. `curl -l` answers
# `/Nintendo/Album/2026`, not `2026`, so appending the raw line built
# `.../Nintendo/Album//Nintendo/Album/2026//` and every fetch missed. Filtered to
# all-digit names too: the album has an `Extra` directory that sorts after the years
# and is not a date.
# RETRIED, BECAUSE sys-ftpd DROPS LISTINGS UNDER REPEATED REQUESTS. tools/deploy.sh
# already records this: consecutive directory listings go unanswered and burn their
# timeout. Walking the album costs three listings back to back, so an empty answer for
# the middle one silently produced a path like `Album/2026/09/` with no day and fetched
# nothing. Three tries with a breath between them is enough.
newest() {
    local out i
    for i in 1 2 3; do
        out=$(curl -s "${CURL_AUTH[@]}" --max-time 30 -l "$1" 2>/dev/null | tr -d '\r' | sed 's|.*/||' | grep -E '^[0-9]+$' | sort -n | tail -1)
        [ -n "$out" ] && { printf '%s\n' "$out"; return 0; }
        sleep 1
    done
    return 1
}

Y=$(newest "$B/Nintendo/Album/")
[ -n "$Y" ] || { echo "  !! /Nintendo/Album is empty -- has anything been captured?"; exit 1; }
M=$(newest "$B/Nintendo/Album/$Y/")
D=$(newest "$B/Nintendo/Album/$Y/$M/")
DIR="Nintendo/Album/$Y/$M/$D"
echo "== newest album day: $DIR =="

listing=$(curl -s "${CURL_AUTH[@]}" --max-time 30 -l "$B/$DIR/" 2>/dev/null | tr -d '\r' | sed 's|.*/||' | sort)
shots=$(printf '%s\n' "$listing" | grep -i '\.jpg$' | tail -"$N")
clips=$(printf '%s\n' "$listing" | grep -i '\.mp4$' | tail -1)

got=0
for f in $shots; do
    printf "  %-44s " "$f"
    if curl -s "${CURL_AUTH[@]}" --max-time 60 -o "$OUT/$f" "$B/$DIR/$f" 2>/dev/null && [ -s "$OUT/$f" ]; then
        echo "$(stat -c%s "$OUT/$f") bytes"
        got=$((got+1))
    else
        echo "FAILED"
        rm -f "$OUT/$f"
    fi
done

if [ "$MODE" = "--frames" ] && [ -n "$clips" ]; then
    printf "  %-44s " "$clips"
    if curl -s "${CURL_AUTH[@]}" --max-time 300 -o "$OUT/$clips" "$B/$DIR/$clips" 2>/dev/null && [ -s "$OUT/$clips" ]; then
        echo "$(stat -c%s "$OUT/$clips") bytes"
        if command -v ffmpeg >/dev/null 2>&1; then
            rm -f "$OUT/frame_"*.jpg
            # 6 fps is enough to see a weapon lag its joint and few enough frames to look at.
            ffmpeg -loglevel error -y -i "$OUT/$clips" -vf fps=6 "$OUT/frame_%03d.jpg" </dev/null
            echo "  split into $(ls "$OUT"/frame_*.jpg 2>/dev/null | wc -l) frame(s) at 6 fps"
        else
            echo "  !! no ffmpeg on PATH -- clip pulled but not split"
        fi
    else
        echo "FAILED"
    fi
fi

echo "  $got still(s) in ${OUT#"$ROOT/"}"
