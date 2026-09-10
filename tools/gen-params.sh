#!/usr/bin/env bash
# Generates the attribute patch: fighter/common/param/fighter_param.prcxml
#
# ARCropolis prcxml patch format: every list index must be present; unchanged entries are the
# scalar placeholder `dummy`.
#
# THE FILE MUST BE UTF-8, NOT UTF-16. ARCropolis parses prcxml with prcx plus xml-rs 0.8, both of
# which are UTF-8 only. Shipped as UTF-16LE with a BOM, this file failed to parse and ARCropolis
# dropped it in silence -- which is also why it never reported a conflict against the George mod's
# fighter_param.prcx. Telemetry caught it: every live attribute was vanilla (weight 79 not 135,
# jump_y 35.5 not 50.51). check.sh now parses the file rather than inspecting its byte order mark,
# which is what let the mistake stand. Note that ui/message/msg_name.xmsbt is the opposite --
# UTF-16 -- so each format is checked against its own real requirement.
#
# EVERY VALUE IS READ FROM THE GAME. The table below names a param and the fighter to take it
# from; tools/prc-tool getparam looks it up in that fighter's own fighter_param_table row,
# resolved by the row's `fighter_kind` field. The previous version hardcoded the numbers under a
# comment claiming they came from the game. They happened to be right -- all 18 verified -- but
# the same habit had already put Cloud's scale at 1.03 against an actual 0.98 and K. Rool's at
# 1.15 against 0.89, so the claim is now enforced rather than asserted. The element type
# (<float>/<int>) is taken from the param's real type for the same reason.
#
# Pikachu is index 7 of fighter_param_table (94 entries), proven by matching the six values
# telemetry read off the live game against all 94 rows -- index 7 is the unique match.
#
# Regenerate with:  bash tools/gen-params.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/pikachu-moveset/mod/fighter/common/param/fighter_param.prcxml"
PRC="$ROOT/extracted/fighter/common/param/fighter_param.prc"
LBL="$ROOT/vendor/param-labels/ParamLabels.csv"
GET="$ROOT/tools/prc-tool/target/release/getparam.exe"
PIKACHU_INDEX=7
TABLE_LEN=94
mkdir -p "$(dirname "$OUT")"
[ -f "$PRC" ] || { echo "no vanilla fighter_param.prc at $PRC" >&2; exit 1; }

# param | fighter to take it from | why
#
# The movement set is the coherent bundle chosen in Round 5 of the design interview. The landing
# lag block is new: each aerial takes its source fighter's own landing_attack_air_frame, because
# leaving Pikachu's meant b-air landed with 18 frames of lag instead of Cloud's 8 and d-air with
# 22 instead of K. Rool's 14 -- the "lag bounded by pikachu's animations" reported in playtest.
ATTRS="
weight|koopa|Bowser
walk_speed_max|marth|Marth
dash_speed|szerosuit|Zero Suit Samus
run_speed_max|littlemac|Little Mac
run_accel_mul|littlemac|Little Mac
ground_brake|littlemac|Little Mac
air_speed_x_stable|purin|Jigglypuff
air_accel_x_mul|purin|Jigglypuff
air_accel_x_add|purin|Jigglypuff
air_speed_y_stable|jack|Joker
dive_speed_y|jack|Joker
air_brake_y|jack|Joker
air_accel_y|falco|chosen gravity, equals Falco's
jump_y|falco|Falco
mini_jump_y|falco|Falco
jump_aerial_y|falco|Falco
jump_initial_y|falco|Falco
jump_speed_x_mul|falco|Falco
landing_attack_air_frame_n|palutena|n-air is Palutena's
landing_attack_air_frame_f|demon|f-air is Kazuya's
landing_attack_air_frame_b|cloud|b-air is Cloud's
landing_attack_air_frame_hi|kamui|u-air is Corrin's
landing_attack_air_frame_lw|krool|d-air is K. Rool's
"

resolve() { # resolve <param> <fighter> -> "value<TAB>type", refusing anything unusable
    local out
    out=$("$GET" "$PRC" "$LBL" "$2" "$1" 2>/dev/null | head -1)
    local val type
    val=$(printf '%s' "$out" | cut -f2)
    type=$(printf '%s' "$out" | cut -f3)
    case "$val" in
        ""|ABSENT|NON_SCALAR)
            echo "REFUSING: $2 has no usable '$1' (got '${val:-nothing}')" >&2; exit 1 ;;
    esac
    case "$type" in
        float|int|uint|byte|sbyte|short|ushort|bool) ;;
        *) echo "REFUSING: '$1' on $2 has unhandled type '$type'" >&2; exit 1 ;;
    esac
    printf '%s\t%s' "$val" "$type"
}

tmp=$(mktemp)
{
    echo '<?xml version="1.0" encoding="utf-8"?>'
    echo '<struct>'
    printf '\t<list hash="fighter_param_table">\n'
    for i in $(seq 0 $((TABLE_LEN - 1))); do
        if [ "$i" -eq "$PIKACHU_INDEX" ]; then
            printf '\t\t<struct index="%s">\n' "$i"
            while IFS='|' read -r key fighter src; do
                [ -z "${key// }" ] && continue
                r=$(resolve "$key" "$fighter")
                v=$(printf '%s' "$r" | cut -f1); t=$(printf '%s' "$r" | cut -f2)
                printf '\t\t\t<%s hash="%s">%s</%s>\n' "$t" "$key" "$v" "$t"
            done <<< "$ATTRS"
            printf '\t\t</struct>\n'
        else
            printf '\t\t<hash40 index="%s">dummy</hash40>\n' "$i"
        fi
    done
    printf '\t</list>\n'
    echo '</struct>'
} > "$tmp"

mv "$tmp" "$OUT"
n=$(grep -cE '<(float|int|uint|byte|sbyte|short|ushort|bool) hash=' "$OUT")
echo "wrote $OUT ($(wc -c < "$OUT") bytes, UTF-8, $n params read from the game)"
