#!/usr/bin/env bash
# Every ACMD the shipped motion_list.bin NAMES for a PORTED motion must be registered by the
# plugin.
#
# WHY THIS EXISTS, AND THE FIRST THING IT FOUND WAS MY OWN MISTAKE. Two commits claimed that
# Incineroar's `SpecialSThrow` and DK's four cargo throws "had no ACMD at all", inferred from their
# absence in `tools/moves.tsv`. Both claims were false: `tools/gen-moves.sh` keeps its OWN inline
# lists and moves.tsv is a parallel manifest that the CHECKS read, so `pikachu_game_specialsthrow`
# and `pikachu_game_throwff` were both already installed and `git show` says so. Adding the rows
# changed documentation and which moves get measured, and nothing else.
#
# Which is exactly why a check that reads the SHIPPED ARTEFACT rather than either list is worth
# having. It found one real gap immediately: `game_speciallwturn` and `game_specialairlwturn`,
# Revenge's turn-around retaliation -- two `ATTACK`s and a `REVERSE_LR` -- named by two motion
# entries whose animations have been shipping all along and installed by nothing.
#
# Nothing else could have caught it: check-acmd-values diffs the scripts that EXIST against the
# dumps, check-hitboxes measures the hitboxes that EXIST, and neither notices a script that was
# never written.
# IT ITERATES OVER THE SHIPPED MOTIONS, NOT OVER `tools/moves.tsv`, and that distinction is the
# whole value of it. The first version of this check walked moves.tsv -- so a move whose row was
# MISSING was not checked, which is precisely the bug it was written to catch. Self-tested by
# deleting the ThrowFF row and watching it still pass. `tools/ported-motions.txt` is the authority
# for what this moveset actually ships an animation for.
# NOT EVERY NAME IS A FAULT. A source fighter may define no `game_` script for a motion at all --
# DK's ShoulderWait.txt has only `sound_` and `expression_` -- and then there is nothing to port
# and nothing to install. So a name only counts as missing when SOME vendored source dump defines
# it, which is what makes this checkable rather than noisy.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
ML=pikachu-moveset/mod/fighter/pikachu/motion/body/c00/motion_list.bin
SRC=vendor/SSBU-Dumped-Scripts/smashline
MLSHOW=tools/motion-patch/target/release/mlshow.exe
rc=0; missing=""; article=""; checked=0

[ -f "$ML" ] || { echo "  FAIL no shipped motion_list.bin"; exit 1; }
[ -f "$MLSHOW" ] || { echo "  FAIL mlshow not built"; exit 1; }

# `<source fighter>:<motion>` per line. BOTH HALVES MATTER: the motion is what mlshow wants, and
# the fighter is what decides whether a named script is portable at all.
# THE FIRST VERSION THREW THE FIGHTER AWAY and asked "does ANY vendored dump define this script".
# That is far too broad and it produced four false positives in one run -- `game_catchpull` is
# defined by Daisy, Lucas, Peach, Pickel and Tan-Tan, none of which this moveset ports a grab from,
# so there was never anything to install. Only the fighter the motion was PORTED FROM can supply
# its script.
rows=$(grep -vE '^[[:space:]]*(#|$)' tools/ported-motions.txt | tr -d '\r')
motions=$(printf '%s\n' "$rows" | sed 's/.*://')
# One mlshow call for all of them -- it takes a list, and 114 separate processes is a minute.
# `motion (0x...)` headers and `game_script <name>` lines interleave, so the motion each script
# belongs to is the last header seen.
pairs=$("$MLSHOW" "$ML" $motions 2>/dev/null | awk '
    /^[a-z_0-9]+ \(0x/ { m = $1; next }
    $1 == "game_script" { print m "\t" $2 }
' | sort -u)

# ONE PASS OVER THE PLUGIN TREE, not one grep per script: the first version recursively grepped
# the whole dump directory for every one of a hundred-odd names and took minutes.
installed=$(grep -rhos '"game_[a-z0-9_]*"' pikachu-moveset/plugin/src/ 2>/dev/null | tr -d '"' | sort -u)

while IFS=$'\t' read -r motion g; do
    [ -z "${g:-}" ] && continue
    # Which fighter did THIS motion come from?
    src=$(printf '%s\n' "$rows" | awk -F: -v m="$motion" '$2==m{print $1; exit}')
    [ -z "$src" ] && continue
    # Does THAT FIGHTER define the script? Only his own agent counts.
    #
    # AN ARTICLE'S AGENT DOES NOT. `game_specialhiclose` and `game_specialhiopen` are defined in
    # `lua2cpp_gamewatch/gamewatch_parachute/`, not in `gamewatch/` -- they are the PARACHUTE's
    # scripts, and they run on the article's own object. Pikachu's synthesised `special_hi_close`
    # entry inherited the script base from the parachute's motion list, so it names a script that
    # no fighter has ever had; installing one on the fighter would not be a port of anything. The
    # first scoped version of this check globbed `lua2cpp_$src/` and reported both, which is a
    # false positive with a real finding hiding inside it: the up-B's parachute is an article this
    # port does not ship, the same class as Incineroar's rope under ADR 0004.
    if ! grep -qs "fn $g(" "$SRC/lua2cpp_$src/$src/"*.txt 2>/dev/null; then
        if grep -rqs "fn $g(" "$SRC/lua2cpp_$src/" 2>/dev/null; then
            article="$article
       $g  ($motion) is defined by an ARTICLE of $src, not by $src -- nothing to install"
        fi
        continue
    fi
    checked=$((checked+1))
    if ! printf '%s\n' "$installed" | grep -qx "$g"; then
        missing="$missing
       $g  ($motion, ported from $src)"
        rc=1
    fi
done <<< "$pairs"

if [ -n "$missing" ]; then
    echo "  FAIL a shipped motion names these game_ scripts, a source dump defines them, and the"
    echo "       plugin installs none of them:$missing"
    echo "       Add a row to tools/moves.tsv -- gen-moves.sh copies the script verbatim."
else
    echo "  ok  every game_ script named by the $(echo "$motions" | wc -w) ported motion(s) and"
    echo "      defined by a source dump is installed ($checked checked)"
fi
if [ -n "$article" ]; then
    echo "  note these named scripts belong to an ARTICLE and are correctly absent:$article"
fi
exit $rc
