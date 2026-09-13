#!/usr/bin/env bash
# Every INIT, EXEC and EXIT line a ported status has in the SOURCE must be registered here.
#
# THIS IS THE CLASS OF BUG THAT HAS COST THE MOST PLAYTEST ROUNDS, three times over, and each time
# the symptom was something a player could see and nothing static could:
#
#   ShoulderWalk's EXEC    one call, `sub_walk_uniq_process_main_common`. Unregistered, so the
#                          three-speed walk chose its speed once at INIT and never again --
#                          "cargo throw can only move the opposite direction of the grab".
#   the carry's EXIT       DK registers one shared exit on all eleven carry statuses. The port
#                          called the same body from each transition instead, so the engine ending
#                          the status -- getting hit, dying, falling -- tore nothing down.
#   the throws' EXIT       `CatchModule::catch_cut`, the one line that releases the catch. Missing
#                          from all four, so Pikachu could not grab again after a cargo throw.
#
# WHY ONLY THESE THREE LINES. `pre`, `main` and `end` are registered for every status in this
# moveset already, and `main_loop` is reached through `sub_shift_status_main` from inside `main`
# rather than registered. `check_dmg` has no smashline line. Init, Exec and Exit are the three that
# carry side effects, are easy to not notice, and have each been missed.
#
# THE SOURCE OF TRUTH IS THE DECOMP TREE: `vendor/ssbu-status/ghidra_scripts/lua2cpp_<f>.nro/<f>/<f>/status/<Status>/`
# contains one `.c` per line the status actually has, so the presence of `exec.c` IS the fact that
# the status has an EXEC line.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
D=vendor/ssbu-status/ghidra_scripts
rc=0; missing=""; checked=0

# RAT_STATUS_KIND_SPECIAL_HI_OPEN IS DELIBERATELY ABSENT FROM THE TABLE BELOW, and its absence is
# a finding rather than an omission. G&W's up-B status set in the decomp is SpecialHi,
# SpecialHiClose and SpecialHiFall: there is no SpecialHiOpen on the FIGHTER at all.
# game_specialhiopen is defined under lua2cpp_gamewatch/gamewatch_parachute/, so the open parachute
# is a state of the ARTICLE, and the port's SPECIAL_HI_OPEN is its own construction standing in for
# an article ADR 0004 excludes. Listing it with a directory that does not exist would report a
# missing decomp forever.
#
# NO BACKTICKS AND NO COMMENTS INSIDE THE STRING. It is a plain double-quoted shell string, so a
# backtick is command substitution -- the first draft put this paragraph inside it and the check
# tried to RUN game_specialhiopen. The loop skips `#` lines anyway, belt and braces.
#
# RAT status constant | source fighter | source status directory
MAP="
RAT_STATUS_KIND_SPECIAL_HI_CLOSE|gamewatch|SpecialHiClose
RAT_STATUS_KIND_SPECIAL_S_SHOULDER|gaogaen|SpecialSShoulder
RAT_STATUS_KIND_SPECIAL_S_LARIAT|gaogaen|SpecialSLariat
RAT_STATUS_KIND_SPECIAL_S_THROW|gaogaen|SpecialSThrow
RAT_STATUS_KIND_SPECIAL_S_END|gaogaen|SpecialSEnd
RAT_STATUS_KIND_SPECIAL_S_FAILURE|gaogaen|SpecialSFailure
RAT_STATUS_KIND_SPECIAL_LW_HIT|gaogaen|SpecialLwHit
RAT_STATUS_KIND_SHOULDER_START|donkey|ShoulderStart
RAT_STATUS_KIND_SHOULDER_WAIT|donkey|ShoulderWait
RAT_STATUS_KIND_SHOULDER_WALK|donkey|ShoulderWalk
RAT_STATUS_KIND_SHOULDER_TURN|donkey|ShoulderTurn
RAT_STATUS_KIND_SHOULDER_FALL|donkey|ShoulderFall
RAT_STATUS_KIND_SHOULDER_LANDING|donkey|ShoulderLanding
RAT_STATUS_KIND_SHOULDER_JUMP_SQUAT|donkey|ShoulderJumpSquat
RAT_STATUS_KIND_SHOULDER_JUMP_SQUAT_B|donkey|ShoulderJumpSquatB
RAT_STATUS_KIND_SHOULDER_JUMP|donkey|ShoulderJump
RAT_STATUS_KIND_SHOULDER_PASS|donkey|ShoulderPass
RAT_STATUS_KIND_THROW_F_F|donkey|ThrowFF
RAT_STATUS_KIND_THROW_F_B|donkey|ThrowFB
RAT_STATUS_KIND_THROW_F_HI|donkey|ThrowFHi
RAT_STATUS_KIND_THROW_F_LW|donkey|ThrowFLw
"

while IFS='|' read -r kind f dir; do
    case "${kind:-}" in '' | \#*) continue ;; esac
    src="$D/lua2cpp_$f.nro/$f/$f/status/$dir"
    if [ ! -d "$src" ]; then
        echo "  FAIL no decomp for $f $dir -- cannot verify $kind"
        rc=1; continue
    fi
    # Is this status registered here at all? An unshipped one is a documented gap, not a fault.
    grep -rqs "$kind," pikachu-moveset/plugin/src/ || continue
    for line in init:Init exec:Exec exit:Exit; do
        file=${line%%:*}; want=${line##*:}
        [ -f "$src/$file.c" ] || continue
        checked=$((checked+1))
        # `.status(Exec, RAT_STATUS_KIND_X,` -- whitespace between is free-form in these files.
        if ! grep -rqsE "\.status\($want,[[:space:]]*$kind," pikachu-moveset/plugin/src/; then
            missing="$missing
       $want on $kind  ($f $dir/$file.c exists)"
            rc=1
        fi
    done
done <<< "$MAP"

if [ -n "$missing" ]; then
    echo "  FAIL the source has these status lines and the port registers none of them:$missing"
else
    echo "  ok  every Init/Exec/Exit the source has is registered ($checked line(s) checked)"
fi
exit $rc
