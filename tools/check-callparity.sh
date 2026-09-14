#!/usr/bin/env bash
# Every engine call the SOURCE makes for a ported move must appear somewhere in the port.
#
# THE CLASS OF BUG THIS EXISTS FOR, found by hand and then generalised: DK's cargo throw swaps
# `KineticModule::change_kinetic` to `GROUND_STOP` the frame he lands, and this port did not, so
# an air throw slid across the floor. Nothing static caught it, because the call is not in a
# status line at all -- `ThrowFF_main` shifts to a loop that is one call to a STATIC,
# `FUN_7100021af0`, and the community status dump contains only status lines.
#
# So the source side here is the union of:
#   * the status directories for the move, from vendor/ssbu-status-all
#   * every `FUN_71xxxxxxxx` static those directories call, decompiled into
#     vendor/decomp/statics/<fighter>.c by tools/decomp-statics.sh
#
# It reports calls the source makes and the port never does, ANYWHERE in the module. That is
# deliberately coarse: a call present in the wrong function is a different and much rarer bug
# than a call that is simply absent, and absent is what has cost the rounds.
#
# NOT A GATE. Several source calls are legitimately absent under a standing decision -- the rope
# ARTICLE calls under ADR 0004, the CSK effects under ADR 0002 -- so this prints a report and a
# count rather than failing a build. Read it; do not automate past it.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
A=vendor/ssbu-status-all/ghidra_scripts

# port module | source fighter | status-directory glob
MAP="
specials/whip|gaogaen|SpecialS
specials/shoulder|donkey|Shoulder
specials/shoulder|donkey|ThrowF
specials/speciallw|gaogaen|SpecialLw
specials/specialhi|gamewatch|SpecialHi
specials/specialhiclose|gamewatch|SpecialHiClose
specials/specialhifall|gamewatch|SpecialHiFall
"

total_missing=0
while IFS='|' read -r mod fighter glob; do
    case "${mod:-}" in '' | \#*) continue ;; esac
    dir="$ROOT/pikachu-moveset/plugin/src/$mod"
    [ -d "$dir" ] || { echo "  (no such module $mod)"; continue; }

    # SOURCE: the status dirs, plus ONLY the statics those dirs actually call.
    #
    # Concatenating the whole fighter's statics was wrong and it showed: Revenge came back
    # "missing" `LinkModule::send_event_parents`, which is the WHIP's release, because both live
    # in gaogaen. A static belongs to the status that reaches it, so the addresses are collected
    # from these directories and the statics file is filtered to them.
    src=$(mktemp)
    for d in "$A/lua2cpp_$fighter.nro/$fighter/$fighter/status/$glob"*; do
        [ -d "$d" ] && cat "$d"/*.c 2>/dev/null
    done > "$src"
    if [ -s "vendor/decomp/statics/$fighter.c" ]; then
        grep -ohE "FUN_71[0-9a-f]{8}" "$src" | sort -u | sed 's/^FUN_/0x/' > "$src.want"
        awk -v want="$src.want" '
            BEGIN { while ((getline a < want) > 0) { WANT[a] = 1 } }
            /^\/\* ==== 0x[0-9a-f]+/ { keep = WANT[$3] }
            keep { print }
        ' "vendor/decomp/statics/$fighter.c" >> "$src"
        rm -f "$src.want"
    fi

    # `_impl` IS THE DECOMPILER'S SUFFIX ON THE LUA_BIND THUNK, and the port calls the wrapper by
    # its bare name. Stripping it is not cosmetic: leaving it on made all 253 calls look missing,
    # which is a report nobody would read twice.
    grep -ohE "[A-Za-z]+Module__[a-z_0-9]+" "$src" | sed 's/__/::/; s/_impl$//' | sort -u > "$src.calls"
    grep -rhoE "[A-Za-z]+Module::[a-z_0-9]+" "$dir" | sort -u > "$src.port"
    missing=$(comm -23 "$src.calls" "$src.port")
    n=$(printf '%s' "$missing" | grep -c . || true)
    total_missing=$((total_missing + n))
    printf '%-28s vs %-9s %-16s  source makes %3d call(s), port is missing %d\n' \
        "$mod" "$fighter" "$glob*" "$(wc -l < "$src.calls")" "$n"
    [ "$n" -gt 0 ] && printf '        %s\n' $missing
    rm -f "$src" "$src.calls" "$src.port"
done <<< "$MAP"
echo
echo "call parity: $total_missing source call(s) with no counterpart in the port"
echo "  (some are correct absences -- ADR 0004 articles, ADR 0002 effects -- read, do not gate)"
