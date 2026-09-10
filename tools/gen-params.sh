#!/usr/bin/env bash
# Generates the attribute patch: fighter/common/param/fighter_param.prcxml
#
# ARCropolis prcxml patch format: every list index must be present; unchanged entries are the
# scalar placeholder `dummy`.
#
# THE FILE MUST BE UTF-8, NOT UTF-16. ARCropolis parses prcxml with `prcx` plus `xml-rs` 0.8,
# both of which are UTF-8 only. Shipped as UTF-16LE with a BOM, this file failed to parse, so
# ARCropolis silently dropped it -- which is also why it never reported a conflict against the
# George mod's fighter_param.prcx. Telemetry caught it: every live attribute was vanilla
# (weight 79 not 135, jump_y 35.5 not 50.51). check.sh now parses the file rather than
# inspecting its byte order mark, which is what let the mistake stand.
# (bomb_single_slots .../ui_chara_db.prcxml).
#
# Pikachu is index 7 of fighter_param_table (94 entries), found with tools/prc-tool.
# Every value below is read from the source fighter's own row in the vanilla
# fighter_param.prc -- not transcribed from a website.
#
# Regenerate with:  bash tools/gen-params.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/pikachu-moveset/mod/fighter/common/param/fighter_param.prcxml"
PIKACHU_INDEX=7
TABLE_LEN=94

mkdir -p "$(dirname "$OUT")"

# param | value | source
ATTRS="
weight|135|Bowser
walk_speed_max|1.575|Marth
dash_speed|2.42|Zero Suit Samus
run_speed_max|2.464|Little Mac
run_accel_mul|0.17798|Little Mac
ground_brake|0.116|Little Mac
air_speed_x_stable|1.332|Jigglypuff
air_accel_x_mul|0.08|Jigglypuff
air_accel_x_add|0.05|Jigglypuff
air_speed_y_stable|1.63|Joker
dive_speed_y|3.097|Joker
air_brake_y|0.015|Joker
air_accel_y|0.13|chosen gravity, equals Falco's
jump_y|50.51|Falco
mini_jump_y|17.34|Falco
jump_aerial_y|50.51|Falco
jump_initial_y|27.7805|Falco
jump_speed_x_mul|0.9|Falco
"

tmp="$(mktemp)"
{
    echo '<?xml version="1.0" encoding="utf-8"?>'
    echo '<struct>'
    echo -e '\t<list hash="fighter_param_table">'
    for i in $(seq 0 $((TABLE_LEN - 1))); do
        if [ "$i" -eq "$PIKACHU_INDEX" ]; then
            echo -e "\t\t<struct index=\"$i\">"
            while IFS='|' read -r key val src; do
                [ -z "${key// }" ] && continue
                echo -e "\t\t\t<float hash=\"$key\">$val</float>"
            done <<< "$ATTRS"
            echo -e "\t\t</struct>"
        else
            echo -e "\t\t<hash40 index=\"$i\">dummy</hash40>"
        fi
    done
    echo -e '\t</list>'
    echo '</struct>'
} > "$tmp"

mv "$tmp" "$OUT"

echo "wrote $OUT ($(wc -c < "$OUT") bytes, UTF-8)"
