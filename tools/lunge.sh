#!/usr/bin/env bash
# Little Mac's f-smash gets its range from a 21.45-unit forward root-motion lunge in his own
# animation. Pikachu's f-smash animation has no root-motion track, so the range is lost. His
# ACMD contains no speed call -- the movement is purely animated -- so we reproduce it with
# kinetic energy, which also carries the hurtbox forward and preserves the move's
# range-for-commitment trade.
#
# Measured Trans z displacement (Little Mac, model units): f8 5.47, f12 15.16, f14 15.91,
# f30 17.71, total span 21.45. Times his scale 1.11 gives ~17.7 world units by frame 14.
#
# SPEED below is a first estimate: ground friction decays the value, so the true displacement
# needs one calibration pass against the telemetry's logged position delta.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
F="$ROOT/pikachu-moveset/plugin/src/smashes/mod.rs"
SPEED=1.8

python - >/dev/null 2>&1 <<'PY' || true
PY

awk -v speed="$SPEED" '
  /^unsafe extern "C" fn pikachu_game_attacks4/ { inf=1; print; next }
  inf && /^\{/ { print; next }
  inf && !done && /^    frame\(agent.lua_state_agent/ {
    print "    // Reproduces Little Mac.s forward lunge, which lives in his animation root"
    print "    // motion and is absent from Pikachu.s f-smash animation. See tools/lunge.sh."
    print "    if macros::is_excute(agent) {"
    print "        macros::SET_SPEED_EX(agent, " speed ", 0.0, *KINETIC_ENERGY_RESERVE_ATTRIBUTE_MAIN);"
    print "    }"
    done=1
  }
  /^\}/ && inf { inf=0; done=0 }
  { print }
' "$F" > "$F.tmp" && mv "$F.tmp" "$F"

echo "lunge inserted at speed $SPEED into:"
grep -c "SET_SPEED_EX" "$F"
