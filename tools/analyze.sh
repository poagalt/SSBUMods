#!/usr/bin/env bash
# Pull everything the console knows about the last session and summarise it. One command.
#
# WHY. Reading a playtest used to be twenty ad-hoc commands: find the log, pull it, resolve the
# motion hashes, grep for each move, list the crash reports, work out which one is new, pull it,
# find the crashed thread, read the backtrace. Every one of those is a chance to look in the wrong
# place, and one of them -- checking a build with `grep "^error"` against cargo's colour codes --
# silently reported success on a failed build.
#
# So it is a script. Each round it gets whatever the last round wished it had.
#
#   bash tools/analyze.sh            pull and summarise
#   bash tools/analyze.sh --keep     also keep a timestamped copy for comparing rounds
#
# Everything lands in work/logs/. The raw log and any crash report are kept verbatim alongside the
# summary, so nothing here has to be trusted -- it can be re-read.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
L=work/logs
mkdir -p "$L"
STAMP=$(date +%Y%m%d-%H%M%S)

echo "== 1. telemetry =="
sz=$(bash tools/ftpq.sh size /rat-telemetry.log)
if [ -z "$sz" ]; then
    echo "  no /rat-telemetry.log on the card."
    echo "  Either the game has not been launched since the last deploy, or the sink could not"
    echo "  write. The install marker is the discriminator: if the plugin loaded at all it writes"
    echo "  'telemetry installed' on the first flush, so an absent file means an absent plugin."
    # A LOCAL COPY FROM A PREVIOUS ROUND IS NOT THIS ROUND'S DATA. Analysing it anyway prints a
    # complete, plausible, WRONG report -- the exact failure this script exists to stop -- so the
    # stale file is moved aside and everything below says "no telemetry" instead.
    if [ -f "$L/rat-resolved.log" ]; then
        mv "$L/rat-resolved.log" "$L/rat-resolved.stale.log"
        mv "$L/rat-telemetry.log" "$L/rat-telemetry.stale.log" 2>/dev/null
        echo "  (a log from an earlier round was moved to $L/rat-resolved.stale.log rather than"
        echo "   analysed -- last round's data reported as this round's is worse than no data)"
    fi
else
    bash tools/ftpq.sh get /rat-telemetry.log > "$L/rat-telemetry.log" 2>/dev/null
    got=$(stat -c%s "$L/rat-telemetry.log" 2>/dev/null || echo 0)
    echo "  pulled $got bytes (card reported $sz)"
    [ "$got" != "$sz" ] && echo "  !! short read -- sys-ftpd truncates under load, re-run to confirm"
    DICT1=vendor/param-labels/ParamLabels.csv DICT2=vendor/Hashes.txt \
        tools/motion-patch/target/release/resolve.exe \
        < "$L/rat-telemetry.log" > "$L/rat-resolved.log" 2>/dev/null
    [ "${1:-}" = "--keep" ] && cp "$L/rat-resolved.log" "$L/round-$STAMP.log"
fi

R="$L/rat-resolved.log"
if [ -s "$R" ]; then
    echo
    echo "== 2. is this log from the CURRENT build? =="
    logged=$(grep -m1 -oE "BUILD [0-9a-f]+\+?[a-z]*" "$R" | sed 's/^BUILD //')
    head=$(git rev-parse --short HEAD 2>/dev/null)
    if [ -z "$logged" ]; then
        echo "  the log has no BUILD line -- it predates build stamping, or the plugin is old"
    elif [ "${logged%%+*}" = "$head" ]; then
        echo "  yes: $logged, and HEAD is $head"
    else
        echo "  NO. The log says $logged and HEAD is $head."
        echo "  This playtest ran a DIFFERENT build. Everything below describes that build, not"
        echo "  the current code -- redeploy and retest before drawing any conclusion from it."
    fi

    echo
    echo "== 2b. self-test: what the console actually loaded =="
    grep -E "telemetry installed|articles:|SELFTEST|object is alive" "$R" | sed 's/^/  /'
    grep -E "PARAMS|EXPECTED" "$R" | head -8 | sed 's/^/  /'

    echo
    echo "== 3. anything that reported a problem =="
    # Every marker the plugin emits for a state it did not expect. Grouped so a clean run prints
    # one line rather than nothing, because "no output" and "nothing wrong" look the same and only
    # one of them is good news.
    bad=$(grep -cE "REFUSED|UNRESOLVED|rejected|failed|no writable path" "$R")
    if [ "$bad" = "0" ]; then
        echo "  none -- no refused motion changes, no invisibility, no unresolved motions"
    else
        grep -E "REFUSED|UNRESOLVED|rejected|failed|no writable path" "$R" | sed 's/^/  /'
    fi

    echo
    echo "== 4. what each move did =="
    for tag in WHIP REVENGE CARGO UPB PROBE; do
        n=$(grep -c "\[rat\] $tag " "$R")
        printf '  %-8s %s line(s)\n' "$tag" "$n"
        [ "$n" != "0" ] && grep "\[rat\] $tag " "$R" | head -25 | sed 's/^/      /'
    done

    echo
    echo "== 5. status timeline (transitions only) =="
    grep "status .* -> " "$R" | sed 's/^/  /' | tail -40

    echo
    echo "== 6. motions that ran, longest first =="
    grep "ended after" "$R" \
      | sed -n 's/.*motion \([a-z_0-9]*\) *ended after \([0-9]*\)f.*/\2 \1/p' \
      | sort -rn | awk '!seen[$2]++' | head -20 | sed 's/^/  /'
fi

echo
echo "== 6b. the PREVIOUS session (rotated .prev) =="
# THE CRASHED RUN IS IN HERE, NOT IN THE CURRENT LOG. A crash closes the game and the next launch
# starts a new log; the sink rotates the old one to `.prev` so it survives. Reading only the
# current file is how round 3 concluded the cargo code never ran -- its evidence had already been
# rotated away, and before the rotation existed it had been overwritten entirely.
prevsz=$(bash tools/ftpq.sh size /rat-telemetry.log.prev)
if [ -z "$prevsz" ]; then
    echo "  none -- this is the first session since the sink started rotating"
else
    bash tools/ftpq.sh get /rat-telemetry.log.prev > "$L/rat-prev.log" 2>/dev/null
    DICT1=vendor/param-labels/ParamLabels.csv DICT2=vendor/Hashes.txt \
        tools/motion-patch/target/release/resolve.exe \
        < "$L/rat-prev.log" > "$L/rat-prev-resolved.log" 2>/dev/null
    echo "  pulled $prevsz bytes; its LAST 12 lines are what that session was doing when it ended:"
    tail -12 "$L/rat-prev-resolved.log" | sed 's/^/    /'
fi

echo
echo "== 7. crash reports =="
# Newest first. The interesting one is any report newer than the newest we already have locally,
# which is why they are mirrored rather than just listed.
mkdir -p "$L/crash"
remote=$(bash tools/ftpq.sh list /atmosphere/crash_reports 2>/dev/null \
         | awk '{print $NF}' | grep '\.log$' | sort)
if [ -z "$remote" ]; then
    echo "  none on the card"
else
    newest=""
    for f in $remote; do
        if [ ! -f "$L/crash/$f" ]; then
            bash tools/ftpq.sh get "/atmosphere/crash_reports/$f" > "$L/crash/$f" 2>/dev/null
            got=$(stat -c%s "$L/crash/$f" 2>/dev/null || echo 0)
            # A ZERO-BYTE PULL IS NOT A REPORT. sys-ftpd drops about one transfer in eighty, and
            # keeping the empty file would mark it "already have it" forever -- so the one report
            # that mattered would be permanently invisible. Delete and try again next run.
            if [ "$got" -lt 1000 ]; then
                rm -f "$L/crash/$f"
                echo "  !! $f came back $got bytes -- dropped, will retry next run"
                continue
            fi
            echo "  NEW $f ($got bytes)"
            newest="$L/crash/$f"
        fi
    done
    [ -z "$newest" ] && echo "  $(echo "$remote" | wc -l) report(s), none new since the last run"
    if [ -n "$newest" ]; then
        echo
        echo "  --- crashed thread of $(basename "$newest") ---"
        # The header plus the crashed thread's registers and its whole return-address chain. The
        # chain is the part that matters and the part that was being skipped: reading only the top
        # frame is how `load_fighter_effects` got blamed for a crash whose real caller was six
        # frames down.
        sed -n '1,20p' "$newest" | grep -E "Type:|Address:|Fault|Thread Name" | sed 's/^/    /'
        awk '/Stack Trace:/{f=1} f&&/ReturnAddress\[/{print} f&&/^ *$/{exit}' "$newest" \
            | head -20 | sed 's/^/    /'
        echo "    (full report: $newest)"
    fi
fi

echo
echo "== 8. what is on the card right now =="
printf '  plugin  local %s  card %s\n' \
  "$(stat -c%s pikachu-moveset/plugin/target/aarch64-skyline-switch/release/librat_of_doom.nro 2>/dev/null)" \
  "$(bash tools/ftpq.sh size /atmosphere/contents/01006A800016E000/romfs/skyline/plugins/librat_of_doom.nro)"
echo "  (tools/deploy.sh verifies all 86 mod files by size; this is just the plugin)"
