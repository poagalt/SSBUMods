#!/usr/bin/env bash
# Parity harness: no hand-written code may use another fighter's namespaced constants.
#
# This exists because the Game & Watch up-B was broken for weeks by exactly that. Ported from the
# George mod, it kept FIGHTER_DIDDY_STATUS_KIND_SPECIAL_HI_CHARGE (0x1F0) and _CHARGE_DAMAGE
# (0x1F1). Fighter EXTRA status kinds are per-fighter: Pikachu's run 0x1E1..0x1EF with NUM = 0x1F0,
# so Diddy's charge status is one past the end of Pikachu's table and the damage one is two past.
# Pikachu was sent to statuses that do not exist and floated in free fall instead of rising.
#
# tools/reach/src/bin/deforeign.rs already strips this class of constant from GENERATED scripts --
# it is the same reason the ported f-tilt was setting Pikachu's FLAG_STRANS_1 and the d-air his
# FLAG_PAUSE. But the specials are hand-written, so deforeign never saw them and nothing else did
# either. A rule enforced on generated code and not on hand-written code is not a rule.
#
# PIKACHU and the engine-global namespaces are fine. Everything else is a bug.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/pikachu-moveset/plugin/src"
rc=0

# Every fighter we port from, plus Diddy for the George lineage.
FOREIGN="CHROM ELIGHT FALCO ROBOT LITTLEMAC FOX GAMEWATCH PALUTENA DEMON CLOUD KAMUI KROOL
GAOGAEN MEWTWO KOOPA GANON DONKEY DIDDY PACKUN MARTH SZEROSUIT PURIN JACK"

# TWO KINDS, and only one is a bug. The distinction is the point of this check.
#
#   STATUS_KIND and INSTANCE_WORK_ID are fighter-wide enumerations. A status kind that is valid on
#   one fighter can be past the end of another's table -- Diddy's SPECIAL_HI_CHARGE is 0x1F0 and
#   Pikachu's whole table ends at 0x1F0 -- and an instance work id addresses fighter-wide state, so
#   K. Rool's belly-armor flag is Pikachu's FLAG_PAUSE. Both are real bugs.
#
#   STATUS_<x>_FLAG and STATUS_<x>_WORK are scoped to a single status's own work area. When we
#   implement that status ourselves and both set and read the same id, it is self-consistent: the
#   id is an offset into whatever the engine allocates for the status we own. Those are allowed.
#
# Comment lines are skipped: the modules deliberately document which foreign ids they avoid and
# why, and flagging that documentation would push it out of the code.
found=0
for f in $FOREIGN; do
    hits=$(grep -rn "FIGHTER_${f}_\|WEAPON_${f}_" \
             "$SRC/specials" "$SRC/articles" "$SRC/sword" "$SRC/telemetry" 2>/dev/null \
           | grep -v ':[0-9]*: *//' \
           | grep -E "FIGHTER_${f}_(STATUS_KIND|INSTANCE_WORK_ID)" || true)
    if [ -n "$hits" ]; then
        printf '%s\n' "$hits" | sed "s|$SRC/||;s|^|  FAIL |"
        found=1
    fi
done
[ "$found" -eq 0 ] || rc=1

if [ "$rc" -eq 0 ]; then
    n=$(grep -rc "FIGHTER_PIKACHU_STATUS_KIND_" "$SRC/specials" -r 2>/dev/null | awk -F: '{s+=$2} END{print s+0}')
    echo "  ok  hand-written code uses no other fighter's constants ($n Pikachu status references)"
else
    echo "  a hand-written module uses another fighter's namespaced constant -- those ids are"
    echo "  per-fighter enumerations and do not mean the same thing on Pikachu"
fi
exit "$rc"
