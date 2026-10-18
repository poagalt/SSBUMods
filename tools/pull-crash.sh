#!/usr/bin/env bash
# Pulls Atmosphere crash reports and the Skyline log off the card, newest first.
#
# Needs FTP, so Smash must NOT be running -- ftpd-classic exits when a game starts. After a crash
# the console is back at HOME, which means ftpd is dead too and has to be relaunched by hand.
#
# Two sources, and they answer different questions:
#
#   /atmosphere/crash_reports/     the Horizon-level abort: exception type, fault address, the
#                                 crashed thread's name and registers, and a backtrace with
#                                 module-relative offsets. This is what identifies WHERE.
#   skyline log / sdcard log       plugin-side println! output, including our own [rat] telemetry
#                                 lines. This is what identifies WHAT WE WERE DOING.
#
# A crash report whose thread is TaskWorker2 and whose fault address is 0 has meant a null deref
# inside smashline's fighter init every time so far -- that is the article-clone signature. A
# crash in a game thread with a real fault address is something else.
#
# usage: bash tools/pull-crash.sh
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
. "$ROOT/tools/console.sh"
HOST=$CONSOLE_HOST
PORT=5000
B="ftp://$HOST:$PORT"

# CREDENTIALS, THE SAME ONES tools/deploy.sh USES. Anonymous access worked for a while and then
# stopped with `430 Access denied`, which reads like a data-connection fault and is not one -- it
# is authentication. deploy.sh has always read this file; these tools never did, so they broke the
# first time the server wanted a login and the error pointed nowhere near the cause.
FTP_USER="${FTP_USER:-poaga}"
FTP_PASS="$(cat "$ROOT/.ftp-credentials.local" 2>/dev/null | tr -d '\r\n')"
CURL_AUTH=()
[ -n "$FTP_PASS" ] && CURL_AUTH=(-u "$FTP_USER:$FTP_PASS")

OUT="$ROOT/work/crash"
mkdir -p "$OUT"

if ! curl -s "${CURL_AUTH[@]}" --connect-timeout 5 --max-time 10 -l "$B/" >/dev/null 2>&1; then
    echo "no FTP on $HOST:$PORT."
    echo "Smash must be closed and ftpd-classic relaunched (hbmenu -> ftpd) before this can run."
    exit 1
fi

pull_dir() {
    local remote="$1" label="$2"
    local enc="${remote// /%20}"
    echo
    echo "== $label ($remote) =="
    local listing
    listing=$(curl -s "${CURL_AUTH[@]}" --max-time 30 -l "$B$enc/" 2>/dev/null | tr -d '\r' | grep -v '^\.\{1,2\}$' || true)
    if [ -z "$listing" ]; then
        echo "  (empty or absent)"
        return
    fi
    # Newest last by name: Atmosphere names reports <timestamp>_<programid>.log, so a plain sort
    # is chronological. Take the last few rather than everything -- these accumulate.
    local n=0
    while read -r f; do
        [ -z "$f" ] && continue
        n=$((n+1))
    done <<< "$listing"
    echo "  $n file(s) on the card; fetching the newest 5"
    printf '%s\n' "$listing" | sort | tail -5 | while read -r f; do
        [ -z "$f" ] && continue
        local fenc="${f// /%20}"
        printf "  %-44s " "$f"
        if curl -s "${CURL_AUTH[@]}" --max-time 60 -o "$OUT/$f" "$B$enc/$fenc" 2>/dev/null; then
            echo "$(wc -c < "$OUT/$f") bytes"
        else
            echo "FAILED"
        fi
    done
}

pull_dir "/atmosphere/crash_reports" "Atmosphere crash reports"
pull_dir "/atmosphere/fatal_errors" "Atmosphere fatal errors"

# Skyline's own log. Location varies by build; try the ones this setup has used.
echo
echo "== Skyline log =="
for p in /skyline/log.txt /atmosphere/logs/skyline.log /skyline.log; do
    enc="${p// /%20}"
    printf "  %-44s " "$p"
    if curl -s "${CURL_AUTH[@]}" --max-time 30 -o "$OUT/skyline-latest.txt" "$B$enc" 2>/dev/null \
       && [ -s "$OUT/skyline-latest.txt" ]; then
        echo "$(wc -c < "$OUT/skyline-latest.txt") bytes"
        break
    fi
    echo "-"
done

echo
echo "pulled into $OUT"
