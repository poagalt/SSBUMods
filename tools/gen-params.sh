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
DUMP="$ROOT/tools/prc-tool/target/release/prcdump.exe"
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
# THE JAB COMBO PARAMS BELOW HAVE TO BE HERE RATHER THAN IN src/params, and finding that out is
# what the second round of jab reports cost.
#
# src/params hooks `WorkModule::get_param_int`, which is the LUA accessor. That was enough to make
# jab 2 and jab 3 exist -- `check_attack_mtrans` is lua2cpp and reads `attack_combo_max` through
# it. It is NOT enough for the two things the playtest then reported, because both are decided by
# NATIVE code that never goes through that accessor:
#
#   * `combo_attack_12_end` / `_13_end` are the combo WINDOW, in frames. Nothing in
#     vendor/decomp/common reads them -- grep the whole tree and there is not one hit -- so the
#     reader is the engine's own ComboModule. At Pikachu's 0 the window never expires; the count
#     is then only ever cleared by `sub_status_AttackCommon` when it REACHES `attack_combo_max`,
#     so the combo state survives indefinitely. Playtest: "his jab stores the 1, 2, 3 state
#     instead of resetting after a certain time frame".
#   * `attack100_type` is the rapid jab's master switch, and it is a BOOL: false on Pikachu, true
#     on Little Mac. `status_Attack_Main_button` refuses BOTH doors into
#     FIGHTER_STATUS_KIND_ATTACK_100 without it -- the hold-to-rapid door tests it directly, and
#     the mash door tests `attack_100_enable_cnt <= ATTACK_WORK_INT_100_COUNT`, which on Pikachu's
#     0 would open on the first frame and on Mac's 4 opens after four. Playtest: "I can't rapid
#     jab with little mac".
#
# This file patches Pikachu's own row of the game's fighter_param_table, so the engine reads these
# the same way it reads his weight. That is what it is for, and it is the only mechanism here that
# native code can see.
#
# `attack_combo_type` IS DELIBERATELY NOT IN THE LIST. It is a hash40 (fighter_combo_type_hit),
# which `resolve` below cannot emit -- and it does not need to be. Every consumer in the decomp
# (`attack_mtrans_post_process`, `sub_status_AttackComboCommon_button`,
# `sub_status_Attack100_common`, `sub_transition_group_check_ground_attack`) is lua2cpp and reads
# it through `get_param_int`, which src/params already answers.
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
attack_combo_max|littlemac|three jabs, not one
combo_attack_12_end|littlemac|the jab-1 window, read natively by ComboModule
combo_attack_13_end|littlemac|the jab-2 window, same
attack100_type|littlemac|the rapid jab exists at all
attack_100_enable_cnt|littlemac|how many mashes open the rapid jab
attack_100_rebound_count|littlemac|how far the rapid jab pushes
attack_combo_type|littlemac|the combo advances on hit, as Mac's does
"

resolve() { # resolve <param> <fighter> -> "value<TAB>type", refusing anything unusable
    local out
    out=$("$GET" "$PRC" "$LBL" "$2" "$1" 2>/dev/null | head -1)
    local val type
    val=$(printf '%s' "$out" | cut -f2)
    type=$(printf '%s' "$out" | cut -f3)
    # A HASH40 PARAM IS NOT A SCALAR AND `getparam` SAYS SO, WHICH IS NOT THE SAME AS UNUSABLE.
    #
    # `attack_combo_type` is `fighter_combo_type_hit` -- a hash, not a number -- and refusing it
    # here is why it spent a round being answered by the lua hook in src/params instead. That was
    # a split brain: the hook answers `WorkModule::get_param_int`, so lua2cpp readers saw Mac's
    # HIT while the engine's NATIVE readers went on seeing Pikachu's own NONE out of the real
    # table. The prcxml format carries hash40 perfectly well -- every unchanged row in this file
    # is `<hash40 index="N">dummy</hash40>` -- so the value belongs here with the other five and
    # the hook keeps nothing.
    #
    # The label comes from prcdump rather than getparam, because getparam reports the type and
    # not the name for a hash.
    if [ "$val" = "NON_SCALAR" ]; then
        local h
        h=$("$DUMP" "$PRC" "$LBL" 2>/dev/null             | awk -v want="fighter_kind_$2" '$0 ~ ("fighter_kind = " want "$"){f=1;next} f&&/fighter_kind = /{exit} f'             | awk -v k="$1" '$1==k { print $3; exit }')
        case "${h:-}" in
            ""|0x*) echo "REFUSING: '$1' on $2 is non-scalar and has no label (got '${h:-nothing}')" >&2; exit 1 ;;
            *) printf '%s	hash40' "$h"; return ;;
        esac
    fi
    case "$val" in
        ""|ABSENT)
            echo "REFUSING: $2 has no usable '$1' (got '${val:-nothing}')" >&2; exit 1 ;;
    esac
    case "$type" in
        float|int|uint|byte|sbyte|short|ushort|bool|hash40) ;;
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
