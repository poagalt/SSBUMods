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

# A FOREIGN FIGHTER'S OWN NATIVE HELPER IS THE SAME BUG WITH NO CONSTANT IN SIGHT.
#
# The rule above catches a foreign CONSTANT. It does not catch a foreign FUNCTION, and one of those
# cost a playtest round with a symptom nobody would have connected to it: the whip's throw called
#
#     FighterSpecializer_Gaogaen::generate_rope(boma)
#
# word for word, by name, because that is what Incineroar's SPECIAL_S_THROW main does. Run on a
# Pikachu it generated PIKACHU's article at Incineroar's rope index, and the bottom of his article
# list is DENGEKIDAMA -- the Thunder Jolt ball. The playtest reported the victim taking damage that
# "lines up exactly with thunder jolt's damage and visuals, and stales at the same rate", which is
# because it was one.
#
# A `FighterSpecializer_<X>` function carries the foreign enumeration INSIDE ITSELF, so there is no
# identifier here to match and no amount of checking constants would have found it. The whole
# namespace is off limits to hand-written code: these are per-fighter helpers operating on
# per-fighter state, and the host is not that fighter.
for f in $FOREIGN; do
    # `FighterSpecializer_Gaogaen` -> `Gaogaen`: the symbol is CamelCase, the list is upper.
    camel=$(printf '%s' "$f" | sed -E 's/^(.)(.*)$/\1\L\2/')
    hits=$(grep -rniE "FighterSpecializer_${camel}::" \
             "$SRC/specials" "$SRC/articles" "$SRC/sword" "$SRC/telemetry" 2>/dev/null \
           | grep -v ':[0-9]*: *//' || true)
    if [ -n "$hits" ]; then
        printf '%s\n' "$hits" | sed "s|$SRC/||;s|^|  FAIL foreign specializer: |"
        rc=1
    fi
done

# THE NEW STATUS KINDS must be past the end of Pikachu's table and distinct from each other.
#
# src/specials/status.rs already asserts the range at compile time, but not uniqueness -- two
# statuses sharing an id would silently install one on top of the other, and the symptom would be
# an up-B that behaves like an Alolan Whip follow-up. Cheap to check here, impossible to debug in
# game. 0x1F0 is FIGHTER_PIKACHU_STATUS_KIND_NUM.
STATUS_RS="$SRC/specials/status.rs"
if [ ! -f "$STATUS_RS" ]; then
    echo "  FAIL src/specials/status.rs is missing -- the new status kinds have no single home"
    rc=1
else
    vals=$(grep -oE '^pub const RAT_STATUS_KIND_[A-Z_0-9]+: i32 = 0x[0-9A-Fa-f]+' "$STATUS_RS" \
           | grep -v RAT_STATUS_KIND_NUM | awk '{print $NF}')
    nvals=$(printf '%s\n' "$vals" | grep -c . || true)
    nuniq=$(printf '%s\n' "$vals" | sort -u | grep -c . || true)
    if [ "$nvals" -ne "$nuniq" ]; then
        echo "  FAIL two RAT_STATUS_KIND_* constants share an id ($nvals declared, $nuniq distinct)"
        rc=1
    fi
    low=$(printf '%s\n' "$vals" | awk 'strtonum($1) < strtonum("0x1F0")' | grep -c . || true)
    if [ "$low" -ne 0 ]; then
        echo "  FAIL $low RAT_STATUS_KIND_* constant(s) below 0x1F0 -- they would shadow one of Pikachu's"
        rc=1
    fi
    [ "$rc" -eq 0 ] && echo "  ok  $nvals new status kinds, all distinct and past Pikachu's 0x1F0"
fi

# STATUS-SCOPED flags deforeign is asked to KEEP must actually survive into the built source.
# Losing G&W's LANDING_ENABLE puts the up-B back to floating above the ground instead of landing,
# and losing Incineroar's THROW_ROPE leaves Alolan Whip with no whip. Both are silent failures.
for k in FIGHTER_GAMEWATCH_STATUS_SPECIAL_HI_FLAG_LANDING_ENABLE \
         FIGHTER_GAMEWATCH_STATUS_SPECIAL_HI_FLAG_PARACHUTE_OPEN \
         FIGHTER_GAMEWATCH_STATUS_SPECIAL_HI_FLAG_DECIDE_DIRECTION \
         FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_THROW_ROPE; do
    n=$( { grep -rl "$k" "$SRC" 2>/dev/null || true; } | grep -c . || true)
    if [ "$n" -eq 0 ]; then
        echo "  FAIL $k was stripped -- add it to deforeign's KEEP_STATUS_IDS"
        rc=1
    fi
done

if [ "$rc" -eq 0 ]; then
    n=$(grep -rc "RAT_STATUS_KIND_" "$SRC/specials" -r 2>/dev/null | awk -F: '{s+=$2} END{print s+0}')
    echo "  ok  hand-written code uses no other fighter's constants ($n new-status references)"
else
    echo "  a hand-written module uses another fighter's namespaced constant -- those ids are"
    echo "  per-fighter enumerations and do not mean the same thing on Pikachu"
fi
exit "$rc"
