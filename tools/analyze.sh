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
    #
    # Each marker here is a state the plugin decided was wrong, so the list grows with every
    # self-check added: TABLE SHORT means the status table did not extend, ABSORB LEAK means
    # Revenge left engine state armed outside its own status, NO VALID VICTIM means a grab
    # handoff had nobody to hand over, STILL-UNRESOLVED means a motion was missing two seconds
    # after load and not merely early.
    MARKERS="REFUSED|UNRESOLVED|STILL-UNRESOLVED|rejected|failed|no writable path"
    MARKERS="$MARKERS|TABLE SHORT|ABSORB LEAK|NO VALID VICTIM|SHIFT STORM|hold ceiling"
    bad=$(grep -cE "$MARKERS" "$R")
    if [ "$bad" = "0" ]; then
        echo "  none -- status table extended, no motions missing, no state left armed"
    else
        grep -E "$MARKERS" "$R" | sed 's/^/  /'
    fi

    echo
    echo "== 4. what each move did =="
    # THE MOVE SUMMARY FIRST, because it is the index into everything else. One line per attempt
    # saying which statuses the move passed through and for how long -- a whip that never grabbed
    # reads "SPECIAL_S 64f -> SPECIAL_S_END 20f -> WAIT" with no THROW in it, which answers the
    # question before any per-frame line is read.
    echo "  --- move summaries (one per attempt) ---"
    grep "[rat] MOVE " "$R" | tail -30 | sed "s/^/      /"
    echo
    for tag in WHIP REVENGE CARGO SHOULDER UPB PROBE; do
        n=$(grep -c "\[rat\] $tag " "$R")
        printf '  %-8s %s line(s)\n' "$tag" "$n"
        [ "$n" != "0" ] && grep "\[rat\] $tag " "$R" | head -25 | sed 's/^/      /'
    done

    echo
    echo "== 5. status timeline (transitions only) =="
    grep "status .* -> " "$R" | sed 's/^/  /' | tail -40

    echo
    echo "== 5b. THE LAST THING THE PLUGIN DID, and any breadcrumbs =="
    # A CRASH IS DIAGNOSED FROM THE END OF THE LOG, and that was the one view this script did not
    # offer. Section 6b shows the tail of the PREVIOUS session, which is right for "it crashed,
    # then I restarted and played more" -- and wrong for "it crashed and I stopped", where the
    # crashed run IS the current log. Both now print.
    #
    # Crumbs are the lines written unbuffered immediately before a call that might not return
    # (telemetry/sink.rs). If the game died inside one, the crumb naming it is the last line here.
    n=$(grep -cE "about to |REFUSED |validated, about to|stays in status" "$R")
    if [ "$n" != "0" ]; then
        echo "  $n breadcrumb(s); the last 8:"
        grep -E "about to |REFUSED |validated, about to|stays in status" "$R" | tail -8 | sed 's/^/    /'
    else
        echo "  no breadcrumbs -- no risky cross-fighter call was reached this session"
    fi
    echo "  final 10 lines of this session's log:"
    tail -10 "$R" | sed 's/^/    /'

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
    # ONLY THE NEWEST FEW, AND WITH RETRIES.
    #
    # This used to try every report it did not already have locally -- twenty-odd of them, each a
    # fresh FTP connection. sys-ftpd wedges after a handful of connections in quick succession
    # (measured in tools/deploy.sh: one directory listing in five picks up the connection and
    # never sends data), so the run after a crash reported TWENTY consecutive zero-byte pulls and
    # dropped every one, including the three that mattered. The crashes had to be fetched by hand.
    #
    # Old reports are not interesting: a crash from a build that no longer exists cannot be
    # diagnosed. Four is enough to cover "it crashed a few times in one session", and three
    # attempts with a pause covers the wedge.
    for f in $(printf '%s\n' $remote | tail -4); do
        if [ ! -f "$L/crash/$f" ]; then
            got=0
            for attempt in 1 2 3; do
                bash tools/ftpq.sh get "/atmosphere/crash_reports/$f" > "$L/crash/$f" 2>/dev/null
                got=$(stat -c%s "$L/crash/$f" 2>/dev/null || echo 0)
                [ "$got" -ge 1000 ] && break
                sleep 2
            done
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
