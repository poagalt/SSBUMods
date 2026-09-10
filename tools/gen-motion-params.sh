#!/usr/bin/env bash
# Generates fighter/common/param/fighter_param_motion.prcxml -- the dodge frame windows.
#
# This closes a gap open since v0.2. The brief specified Fox's dodges, and the motion timings and
# ACMD scripts were done, but the params that actually govern dodge behaviour were never shipped
# because they are NOT in fighter_param.prc: getparam reports every escape_* param ABSENT there
# for both Pikachu and Fox. They live in fighter_param_motion.prc, a separate file with the same
# fighter_param_motion_table shape.
#
# Diffing Pikachu's whole 132-param row against Fox's showed all fourteen escape_* params differ,
# not just one -- intangibility windows, cancel frames and the post-airdodge stiffness are each
# 1 to 34 frames apart. Pikachu's airdodge cancel window is 55 frames against Fox's 40.
#
# Deliberately NOT taken from Fox, despite differing in that same diff:
#   - the capture/grab offsets other fighters use to hold Pikachu (ganon_special_*, pikmin_catch_*,
#     ryu_final_*, butterflynet_*, snake_capture_*, wiifit_capture_*, rockman_capture_*,
#     gaogaen_rope_*, captain_special_hi_*, cloud_final_*, pickel_thrown_*). Those describe where
#     Pikachu's BODY sits in someone else's move, so they must stay his.
#   - walk/run/item animation frame markers, `flip`, `motion_share`, `capture_motion_height`,
#     `cliff_hang_check_length`, `stop_ceil_speed_y_threshold`. Character-wide traits outside the
#     brief's attribute list, and ADR 0001 excludes those by decision.
#
# UTF-8, like fighter_param.prcxml and unlike ui/message/msg_name.xmsbt. Values are read from the
# game rather than transcribed, and the element type comes from the param's real type.
#
# Regenerate with:  bash tools/gen-motion-params.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/pikachu-moveset/mod/fighter/common/param/fighter_param_motion.prcxml"
PRC="$ROOT/extracted/fighter/common/param/fighter_param_motion.prc"
LBL="$ROOT/vendor/param-labels/ParamLabels.csv"
GET="$ROOT/tools/prc-tool/target/release/motionparam.exe"
SRC=fox
PIKACHU_INDEX=7
TABLE_LEN=94
mkdir -p "$(dirname "$OUT")"
[ -f "$PRC" ] || { echo "no fighter_param_motion.prc at $PRC -- extract it with tools/arc-pull" >&2; exit 1; }

PARAMS="
escape_air_cancel_frame
escape_air_stiff_frame
escape_air_slide_stiff_frame
escape_air_slide_d_stiff_frame
escape_air_slide_u_stiff_frame
escape_attack_frame
escape_b_cancel_frame
escape_b_hit_normal_frame
escape_f_cancel_frame
escape_f_hit_normal_frame
escape_f_penalty_hit_normal_frame
escape_n_cancel_frame
escape_n_hit_normal_frame
escape_n_penalty_hit_normal_frame
"

resolve() { # resolve <param> -> "value<TAB>type", refusing anything unusable
    local out val type
    out=$("$GET" "$PRC" "$LBL" "$SRC" "$1" 2>/dev/null | head -1)
    val=$(printf '%s' "$out" | cut -f2)
    type=$(printf '%s' "$out" | cut -f3)
    case "$val" in
        ""|ABSENT|NON_SCALAR) echo "REFUSING: $SRC has no usable '$1' (got '${val:-nothing}')" >&2; exit 1 ;;
    esac
    case "$type" in
        float|int|uint|byte|sbyte|short|ushort|bool) ;;
        *) echo "REFUSING: '$1' on $SRC has unhandled type '$type'" >&2; exit 1 ;;
    esac
    printf '%s\t%s' "$val" "$type"
}

tmp=$(mktemp)
{
    echo '<?xml version="1.0" encoding="utf-8"?>'
    echo '<struct>'
    printf '\t<list hash="fighter_param_motion_table">\n'
    for i in $(seq 0 $((TABLE_LEN - 1))); do
        if [ "$i" -eq "$PIKACHU_INDEX" ]; then
            printf '\t\t<struct index="%s">\n' "$i"
            while read -r key; do
                [ -z "${key// }" ] && continue
                r=$(resolve "$key")
                v=$(printf '%s' "$r" | cut -f1); t=$(printf '%s' "$r" | cut -f2)
                printf '\t\t\t<%s hash="%s">%s</%s>\n' "$t" "$key" "$v" "$t"
            done <<< "$PARAMS"
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
echo "wrote $OUT ($(wc -c < "$OUT") bytes, UTF-8, $n dodge params read from $SRC)"
