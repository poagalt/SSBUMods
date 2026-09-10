#!/usr/bin/env bash
# Little Mac's f-smash gets its range from a forward root-motion lunge in his own animation.
# Pikachu's f-smash animation has no root-motion track, so the range is lost, and Mac's ACMD
# contains no speed call at all -- the movement is purely animated. So it has to be synthesised.
#
# Measured Trans z displacement (Little Mac, model units):
#   f8 = 5.47   f12 = 15.16   f14 = 15.91   f30 = 17.71
# The lunge therefore runs roughly f5..f14, i.e. it is already underway when the swing starts,
# and covers ~10.4 model units (~11.6 world at his 1.11 scale) between f8 and f14.
#
# FIRST ATTEMPT FAILED: the speed was set before the first frame() call, i.e. on frame 1. A
# smash attack holds its charge at frame 5, so by the time the swing executes on frame 14 the
# speed had long decayed. It is now applied at frame 8, when the swing actually begins, and
# cleared when the hitboxes are cleared.
#
# SPEED is still an estimate because ground friction decays it; telemetry logs the resulting
# `moved x` per motion so it can be calibrated against the ~11.6 world-unit target.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
F="$ROOT/pikachu-moveset/plugin/src/smashes/mod.rs"
SPEED=3.0

awk -v speed="$SPEED" '
  /^unsafe extern "C" fn pikachu_game_attacks4/ { inf = 1; armed = 0 }
  /^unsafe extern "C" fn pikachu_game_attackhi4/ { inf = 0 }
  /^unsafe extern "C" fn pikachu_game_attacklw4/ { inf = 0 }
  { print }
  # Apply the lunge where the swing begins. The down-angled f-smash starts a frame later
  # (UFD: super armor on frame 8-15, 9-15 for the down angled version), so match either 8 or 9
  # -- hardcoding 8.0 silently skipped attacks4lw entirely.
  inf && !armed && /frame\(agent\.lua_state_agent, [89]\.0\);/ {
    print "    // Synthesised Little Mac lunge: his 21.45-unit forward root motion lives in his"
    print "    // animation and Pikachu\x27s f-smash animation has none. See tools/lunge.sh."
    print "    if macros::is_excute(agent) {"
    print "        macros::SET_SPEED_EX(agent, " speed ", 0.0, *KINETIC_ENERGY_RESERVE_ATTRIBUTE_MAIN);"
    print "    }"
    armed = 1
  }
  # and stop it when the hitboxes go away
  inf && armed == 1 && /AttackModule::clear_all/ {
    print "    }"
    print "    if macros::is_excute(agent) {"
    print "        macros::SET_SPEED_EX(agent, 0.0, 0.0, *KINETIC_ENERGY_RESERVE_ATTRIBUTE_MAIN);"
    armed = 2
  }
' "$F" > "$F.tmp" && mv "$F.tmp" "$F"

n=$(grep -c "SET_SPEED_EX" "$F")
echo "lunge: $n SET_SPEED_EX calls inserted at speed $SPEED (expect 6: start+stop x3 angles)"
