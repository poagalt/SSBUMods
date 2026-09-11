#!/usr/bin/env bash
# EVERY ENGINE CALL THE SOURCE FIGHTER'S STATUS MAKES, AND WHETHER THIS PORT MAKES IT TOO.
#
# tools/check-srcparams.sh asks the same question about the source fighter's PARAMS and it has
# earned its keep several times over. This asks it about the source fighter's CODE, which is the
# larger half and was never checked at all.
#
# WHY IT MATTERS MORE THAN IT LOOKS. Every round of playtesting on the grabs found the same shape
# of bug: a native call the port simply did not make. Not a wrong value, not a wrong status graph
# -- a missing line.
#
#   Alolan Whip   CaptureModule::capture was never called, so the victim's own status found no
#                 capture, read no params off us and started a zero-frame clatter. "The opponent
#                 can still attack while the grab is happening."
#   Cargo carry   LinkModule::send_event_nodes(LINK_NO_CAPTURE, "shoulder") was never sent, so the
#                 victim was never told to take the carried pose, and CatchModule::catch_cut was
#                 never called on the way out. "A spinning arms animation that looks like an
#                 animation that would happen when mario is falling."
#
# Both were found by reading one decompiled function closely, after the symptom. There are 30
# decompiled gaogaen statuses, 11 donkey shoulder statuses and 3 gamewatch ones in vendor, and
# reading all of them closely by hand is not a plan. This is that reading, done mechanically.
#
# IT IS A SUGGESTION LIST, NOT A FAILURE LIST, and it exits 0 on purpose. A missing call can be
# entirely correct: half of what these statuses do is presentation this project defers by design
# (EffectModule, SoundModule, the KO cinematic camera), and some is fighter-specific machinery
# there is no Pikachu equivalent for. The value is that the list is COMPLETE and has to be looked
# at, not that every line on it is a bug.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
D=vendor/ssbu-status/ghidra_scripts

# Deferred by design, so they are counted and not listed. docs/decisions.md and ADR 0002 cover
# the effects rule; the visual layer as a whole is BACKLOG.md section 6.
IGNORE='^(EffectModule|SoundModule|CameraModule|ColorBlendModule|VisibilityModule|ModelModule|SlowModule|ZoomModule|ControllerModule|InkPaintModule|TurnModule|StopModule)$'

# source fighter | status directory | our module directory | what it drives
TABLE="
gaogaen|SpecialS|specials|Alolan Whip, the lunge and catch
gaogaen|SpecialSThrow|specials|Alolan Whip, the rope throw
gaogaen|SpecialSShoulder|specials|Alolan Whip, the back body drop
gaogaen|SpecialSLariat|specials|Alolan Whip, the lariat
gaogaen|SpecialSEnd|specials|Alolan Whip, the recovery
gaogaen|SpecialSFailure|specials|Alolan Whip, the whiff
gaogaen|SpecialLw|speciallw|Revenge, the stance
gaogaen|SpecialLwHit|speciallw|Revenge, the retaliation
donkey|ShoulderStart|shoulder|cargo carry, the lift
donkey|ShoulderWait|shoulder|cargo carry, standing
donkey|ShoulderWalk|shoulder|cargo carry, walking
donkey|ShoulderTurn|shoulder|cargo carry, turning
donkey|ShoulderFall|shoulder|cargo carry, airborne
donkey|ShoulderLanding|shoulder|cargo carry, landing
donkey|ThrowFF|shoulder|cargo throw, forward
donkey|ThrowFB|shoulder|cargo throw, back
donkey|ThrowFHi|shoulder|cargo throw, up
donkey|ThrowFLw|shoulder|cargo throw, down
gamewatch|SpecialHi|specialhi|up-B, the rise
gamewatch|SpecialHiClose|specialhiclose|up-B, the parachute closing
gamewatch|SpecialHiFall|specialhifall|up-B, the drift down
"

total_missing=0
report=""
while IFS='|' read -r fighter status dir what; do
    [ -z "${fighter// }" ] && continue
    src="$D/lua2cpp_$fighter.nro/$fighter/$fighter/status/$status"
    if [ ! -d "$src" ]; then
        printf '  %-34s no decompiled status at %s\n' "$what" "$src"
        continue
    fi
    mod="pikachu-moveset/plugin/src/specials/$dir"
    if [ ! -d "$mod" ]; then
        printf '  %-34s no module at %s\n' "$what" "$mod"
        continue
    fi
    # The port may implement one source status across several of our modules, and the shared
    # scaffolding lives a level up, so the whole specials tree counts as "ours".
    # COMMENTS STRIPPED FIRST, or the check is gameable by prose. A doc comment that explains why
    # a call is not made mentions the call by name, and a plain text search then counts it as
    # made -- which is the exact failure tools/check-srcparams.sh had to solve with an explicit
    # `NOT MODELLED:` declaration. Writing about a call is not making it.
    ours="$(cat pikachu-moveset/plugin/src/specials/*.rs pikachu-moveset/plugin/src/specials/*/*.rs \
            pikachu-moveset/plugin/src/*.rs 2>/dev/null | grep -v "^[[:space:]]*//")"
    oursfile="$(mktemp)"
    printf '%s' "$ours" > "$oursfile"

    native="$(grep -ho 'app::lua_bind::[A-Za-z0-9_]*__[a-z0-9_]*_impl' "$src"/*.c 2>/dev/null \
              | sed 's/app::lua_bind:://; s/_impl$//' | sort -u)"
    missing=""; have=0; deferred=0
    while read -r sym; do
        [ -z "$sym" ] && continue
        module="${sym%%__*}"
        method="${sym#*__}"
        if [[ "$module" =~ $IGNORE ]]; then
            deferred=$((deferred+1)); continue
        fi
        # `Module::method` anywhere in the specials tree counts as reproduced. Coarse on purpose:
        # the question is whether the port knows about the call at all.
        if grep -q "${module}::${method}\b" "$oursfile"; then
            have=$((have+1))
        else
            missing="$missing ${module}::${method}"
        fi
    done <<< "$native"
    rm -f "$oursfile"

    n=$(printf '%s' "$missing" | wc -w)
    total_missing=$((total_missing+n))
    printf '  %-34s %2d made, %2d deferred, %2d not made\n' "$what" "$have" "$deferred" "$n"
    if [ -n "$missing" ]; then
        report="$report
    $what:$missing"
    fi
done <<< "$TABLE"

if [ -n "$report" ]; then
    echo
    echo "  calls the source status makes and this port does not:"
    printf '%s\n' "$report" | fold -s -w 108 | sed 's/^/  /'
fi
echo
echo "  $total_missing unmade call(s) across all ported statuses. This check never fails; it is a"
echo "  reading list. Deferred modules (effects, sound, camera, model) are counted, not listed."
