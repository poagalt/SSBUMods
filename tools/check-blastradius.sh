#!/usr/bin/env bash
# Nothing this mod does may reach a fighter that is not Pikachu.
#
# THE BUG THIS EXISTS FOR. `telemetry::pikachu_telemetry` is a Main line callback installed on
# SOURCE_AGENTS -- gaogaen, donkey, gamewatch, packun -- so that a trace of the REAL move can be
# diffed against ours. That is worth having. What it also meant was that
# `retarget_forward_throw` wrote RAT_STATUS_KIND_SHOULDER_START (0x1F7) into native Donkey Kong's
# `globalTable[0x45]` every frame, and 0x1F7 on DK is THROW_F_HI -- so his forward cargo throw ran
# his throw-UP script from a catch status and the game died. Playtest: "I went to play native dk
# and the game crashed when I tried to cargo throw a mario".
#
# See pikachu-moveset/docs/the-blast-radius.md for the full audit. This checks the three things
# that can regress silently.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
SRC=pikachu-moveset/plugin/src
fail=0

echo "== 1. the shared per-frame callback mutates nothing outside the fighter-kind gate =="
# Every call the callback makes, from its head to the `mine` test, must be a read. The gate itself
# is the anchor: anything textually BEFORE it may not touch state.
head=$(awk '/^unsafe extern "C" fn pikachu_telemetry/,/let mine = /' $SRC/telemetry/mod.rs)
bad=$(printf '%s' "$head" | grep -nE "crate::(victim|specials)::|::(set_[a-z_]+|on_flag|off_flag|change_[a-z_]+)\(" || true)
if [ -n "$bad" ]; then
    echo "  FAIL these run before the fighter-kind gate:"; printf '       %s\n' $bad; fail=1
else
    echo "  ok  nothing before the gate touches state"
fi
# And every mutating call in the body must be inside an `if mine` block. Checked by requiring that
# the three known ones are indented past the gate.
for call in "crate::victim::new_frame" "crate::victim::tick_hold" "crate::victim::tick_deferred_cut" \
            "crate::specials::shoulder::retarget_forward_throw" "moves::tick"; do
    if grep -qE "^        ${call//::/::}" $SRC/telemetry/mod.rs; then
        :
    else
        echo "  FAIL $call is not inside an \`if mine\` block (expected 8-space indent)"; fail=1
    fi
done
[ $fail -eq 0 ] && echo "  ok  all five state-touching calls are gated"

echo; echo "== 2. no agent other than pikachu gets anything but a line callback =="
# SOURCE_AGENTS may only ever receive `.on_line`. A `.status`, `.game_acmd` or `.on_start` on a
# foreign agent would run OUR code as THEIR move.
foreign=$(awk '/for name in SOURCE_AGENTS/,/\.install\(\)/' $SRC/telemetry/mod.rs \
          | grep -oE "\.(status|game_acmd|effect_acmd|sound_acmd|expression_acmd|on_start|on_init|on_fini|on_end)\(" || true)
if [ -n "$foreign" ]; then
    echo "  FAIL SOURCE_AGENTS receive more than a line callback:"; printf '       %s\n' $foreign; fail=1
else
    echo "  ok  SOURCE_AGENTS receive on_line only"
fi
others=$(grep -rhoE 'Agent::new\("[a-z_]+"\)' $SRC | grep -v '"pikachu"' | sort -u || true)
if [ -n "$others" ]; then
    echo "  FAIL a non-pikachu agent is constructed by name:"; printf '       %s\n' $others; fail=1
else
    echo "  ok  every named agent is pikachu"
fi

echo; echo "== 3. the global param hooks still gate on the object before the fighter kind =="
# `is_pikachu` is not the first test on purpose -- an id/object compare that is false for almost
# every call in the game comes first, and `&&` short-circuits so a foreign fighter is never even
# asked its kind. Losing that order would make a hot hook call get_kind on every param read.
ungated=$(grep -nE "^\s*if is_pikachu\(boma\)" $SRC/params/mod.rs || true)
if [ -n "$ungated" ]; then
    echo "  FAIL a hook tests the fighter kind before narrowing on the id/object:"
    printf '       %s\n' $ungated; fail=1
else
    echo "  ok  every hook narrows on id/object first"
fi
n=$(grep -cE "^#\[skyline::hook" $SRC/params/mod.rs)
g=$(grep -cE "is_pikachu\(boma\)" $SRC/params/mod.rs)
if [ "$g" -lt "$n" ]; then
    echo "  FAIL $n hook(s) but only $g fighter-kind guard(s)"; fail=1
else
    echo "  ok  $n global hook(s), $g fighter-kind guard(s)"
fi

echo; echo "== 4. the three debug instruments are off =="
for f in exedump memserve enginetrace; do
    v=$(grep -oE "pub const ENABLED: bool = (true|false)" $SRC/telemetry/$f.rs | grep -oE "(true|false)")
    if [ "$v" = "true" ]; then
        echo "  FAIL $f is ENABLED -- it replaces engine functions or stalls frames for EVERY fighter"
        fail=1
    else
        echo "  ok  $f off"
    fi
done

echo; echo "== 5. the common param patches touch Pikachu's index only =="
for f in fighter_param fighter_param_motion; do
    P=pikachu-moveset/mod/fighter/common/param/$f.prcxml
    idx=$(grep -oE '<struct index="[0-9]+"' "$P" | grep -oE '[0-9]+' | sort -un | tr '\n' ' ')
    if [ "$(echo $idx)" = "7" ]; then
        echo "  ok  $f.prcxml: struct at index 7 only (Pikachu)"
    else
        echo "  FAIL $f.prcxml has structs at indices: $idx -- index 7 is Pikachu, the rest are"
        echo "       other fighters and vanilla has a 369-param struct at every one of them"
        fail=1
    fi
done

echo
if [ $fail -ne 0 ]; then
    echo "BLAST RADIUS CHECK FAILED"; exit 1
fi
echo "blast radius: OK -- nothing reaches a fighter that is not Pikachu"
