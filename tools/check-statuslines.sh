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

# AND EVERY NEW STATUS THAT REGISTERS `End` MUST REGISTER `Exit` TOO, BECAUSE `End` NEVER ARRIVES.
#
# THE BUG THAT COST THE MOST ROUNDS OF ALL, and unlike the three above it was registered correctly
# the whole time. `.status(End, RAT_STATUS_KIND_SPECIAL_S_THROW, ...)` is exactly what gaogaen
# has -- `SpecialSThrow/end.c` exists and there is no `exit.c` -- and the engine simply never
# called it for a status id past Pikachu's own count:
#
#   whip catches logged, pt19 + pt21 + pt22               ~100
#   `dropped the capture (whip over)`, from the End body      0
#   the carry's releases, from an `Exit` body                15
#
# So the whip never released its victim, the victim kept `is_capture` forever, and no grab could
# start -- "after hitting a cargo throw or alolan whip in any capacity modified pikachu cannot grab
# the opponent", for three rounds, with the release code sitting there correct and unreached. Two
# more were dead the same way and nobody had noticed: BOTH up-B statuses exhaust the jump counter
# in their end line, so modified Pikachu kept a double jump G&W does not have.
#
# `Exit` IS THE SAME MOMENT, NOT A DIFFERENT BEHAVIOUR. Both lines dispatch from one engine
# function: smashline hooks End at +0x689c and Exit at +0x6950, and both offsets fall inside
# `L2CFighterBase::sub_end_status_msc` (0x7100006730 -> 0x71000069f0) in
# vendor/main/common-nro-symbols.txt.
#
# THIS SCANS THE REGISTRATIONS, NOT THE DECOMP, and that is the point. Checking `end.c` against
# MAP would have missed RAT_STATUS_KIND_SPECIAL_HI_OPEN entirely -- it is the port's own
# construction standing in for G&W's parachute article, so it has no source directory to look in,
# and its end line was just as dead.
#
# Exempt, with reasons, because the rule is about bodies that DO something:
#   SPECIAL_S_END    gaogaen's end line only retires the ROPE article, which ADR 0004 excludes, so
#                    the port's body is an empty `false.into()` and has nothing to reach.
#   SPECIAL_LW_HIT   `pikachu_speciallw_end` is registered on this as well as on the VANILLA
#                    SPECIAL_LW, whose End does fire. gaogaen's own `SpecialLwHit_end` is a bare
#                    `return 0` -- he reverts the absorb on the way OUT of SpecialLw, before
#                    SpecialLwHit is ever entered -- so this registration is redundant either way.
EXIT_EXEMPT="RAT_STATUS_KIND_SPECIAL_S_END RAT_STATUS_KIND_SPECIAL_LW_HIT"

end_missing=""; end_checked=0
for kind in $(grep -rhoE "\.status\(End,[[:space:]]*RAT_STATUS_KIND_[A-Z_0-9]+," \
                  pikachu-moveset/plugin/src/ | grep -oE "RAT_STATUS_KIND_[A-Z_0-9]+" | sort -u); do
    case " $EXIT_EXEMPT " in *" $kind "*) continue ;; esac
    end_checked=$((end_checked+1))
    if ! grep -rqsE "\.status\(Exit,[[:space:]]*$kind," pikachu-moveset/plugin/src/; then
        end_missing="$end_missing
       Exit on $kind  (it registers End, and End does not fire for a new status)"
        rc=1
    fi
done

if [ -n "$end_missing" ]; then
    echo "  FAIL these new statuses register End with no Exit, so their end body never runs:$end_missing"
else
    echo "  ok  every new status registering End also registers Exit ($end_checked checked)"
fi

exit $rc
