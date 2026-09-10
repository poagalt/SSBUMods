#!/usr/bin/env bash
# Reports, per moveset slot, what is actually implemented -- checked against the shipped files and
# the generated source, not against memory.
#
# Five independent facts per slot:
#   acmd   the game_* script is registered with smashline
#   fx     an effect_* override is registered, so Pikachu's own effects do not play on the move
#   anim   the source fighter's animation is shipped in the mod folder
#   faf    the motion entry's cancel_frame matches the source's (tools/motion-patch)
#   hitbox the move appears in the trace at 0.00 world units from the source
#
# A blank means the fact is false, not unknown. Slots the brief leaves on Pikachu are marked
# "vanilla" and are correct as-is.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
SRC=pikachu-moveset/plugin/src
ANIM=pikachu-moveset/mod/fighter/pikachu/motion/body/c00
ML=$ANIM/motion_list.bin
TRACE=$(bash tools/trace-all.sh 2>/dev/null | grep -v "^Parse:")

# slot | acmd fn | animation file | motion entry | source | trace label
ROWS="
Jab|attack11|c00attack11.nuanmb|attack_11|chrom|jab_chrom
Dash attack|attackdash|c00attackdash.nuanmb|attack_dash|elight|dash_attack_mythra
F-tilt|attacks3|c01attacks3s.nuanmb|attack_s3_s|elight|ftilt_mythra
U-tilt|attackhi3|c02attackhi3.nuanmb|attack_hi3|falco|utilt_falco
D-tilt|attacklw3|c02attacklw3.nuanmb|attack_lw3|robot|dtilt_rob
F-smash|attacks4|c03attacks4s.nuanmb|attack_s4_s|littlemac|fsmash_little_mac
F-smash up|attacks4hi|c03attacks4hi.nuanmb|attack_s4_hi|littlemac|fsmash_up_mac
F-smash down|attacks4lw|c03attacks4lw.nuanmb|attack_s4_lw|littlemac|fsmash_down_mac
U-smash|attackhi4|c04attackhi4.nuanmb|attack_hi4|fox|usmash_fox
D-smash|attacklw4|c04attacklw4.nuanmb|attack_lw4|gamewatch|dsmash_gw
N-air|attackairn|c05attackairn.nuanmb|attack_air_n|palutena|nair_palutena
F-air|attackairf|c05attackairf.nuanmb|attack_air_f|demon|fair_kazuya
B-air|attackairb|c05attackairb.nuanmb|attack_air_b|cloud|bair_cloud
U-air|attackairhi|c05attackairhi.nuanmb|attack_air_hi|kamui|uair_corrin
D-air|attackairlw|c05attackairlw.nuanmb|attack_air_lw|krool|dair_k_rool
Grab|catch|e00catch.nuanmb|catch|palutena|-
Pummel|catchattack|e00catchattack.nuanmb|catch_attack|demon|pummel_kazuya
B-throw|throwb|e01throwb.nuanmb|throw_b|gaogaen|bthrow_incineroar
U-throw|throwhi|e01throwhi.nuanmb|throw_hi|mewtwo|uthrow_mewtwo
Get-up up|downattacku|f03downattacku.nuanmb|down_attack_u|koopa|getup_up_bowser
Get-up down|downattackd|f04downattackd.nuanmb|down_attack_d|koopa|getup_down_bowser
Trip attack|slipattack|g03slipattack.nuanmb|slip_attack|koopa|trip_bowser
Ledge attack|cliffattack|g02cliffattack.nuanmb|cliff_attack_quick|ganon|ledge_ganon
Air dodge|escapeair|b02escapeair.nuanmb|escape_air|fox|-
Forward roll|escapef|b02escapef.nuanmb|escape_f|fox|-
"

printf "  %-14s %-10s %-5s %-4s %-5s %-4s %s\n" "slot" "source" "acmd" "fx" "anim" "faf" "hitbox"
ok=0; total=0
while IFS='|' read -r slot fn anim entry src label; do
    [ -z "${slot// }" ] && continue
    total=$((total+1))
    a=""; grep -qh "game_acmd(\"game_$fn\"" $SRC/*/mod.rs 2>/dev/null && a="yes"
    x=""; grep -qh "effect_acmd(\"effect_$fn\"" $SRC/*/mod.rs 2>/dev/null && x="yes"
    n=""; [ -f "$ANIM/$anim" ] && n="yes"
    f=""
    if [ -f "$ML" ]; then
        # Match the row by motion name. A fixed line number was wrong: with one motion queried
        # the row is line 2, not 3, so every faf came back blank and the summary read "0 of 25"
        # while the FAF patch was in fact correct. A status table that under-reports is as
        # misleading as one that over-reports.
        out=$(tools/motion-patch/target/release/dropin.exe extracted "$ML" "$src:$entry" 2>/dev/null \
              | awk -v m="$entry" '$1 == m')
        sf=$(printf '%s' "$out" | awk '{print $(NF-1)}')
        of=$(printf '%s' "$out" | awk '{print $NF}')
        [ -n "$sf" ] && [ "$sf" = "$of" ] && f="yes"
    fi
    h="-"
    if [ "$label" != "-" ]; then
        h="?"
        printf '%s\n' "$TRACE" | grep -q "^  $label .*worst 0.00 " && h="0.00"
        # A move whose source script has no ATTACK hitboxes is not unverified: Mewtwo's u-throw
        # does its damage through a throw collision, so there is nothing to measure.
        printf '%s\n' "$TRACE" | grep -q "^  $label .*nothing to measure" && h="n/a"
    fi
    printf "  %-14s %-10s %-5s %-4s %-5s %-4s %s\n" "$slot" "$src" "$a" "$x" "$n" "$f" "$h"
    [ -n "$a" ] && [ -n "$x" ] && [ -n "$n" ] && [ -n "$f" ] && ok=$((ok+1))
done <<< "$ROWS"

echo
echo "  $ok of $total slots have script, effect override, animation and frame data all in place"

echo
echo "  Specials -- status machines, so the columns mean something different:"
printf "  %-16s %-10s %-6s %-6s %-6s %s\n" "move" "source" "acmd" "status" "anim" "notes"
spec_row() { # spec_row <label> <source> <acmd fn> <status fn grep> <animation file> <notes>
    local a="" s="" n=""
    grep -qh "game_acmd(\"game_$3\"" "$SRC"/*/mod.rs 2>/dev/null && a="yes"
    grep -rqh "$4" "$SRC"/specials 2>/dev/null && s="yes"
    [ -f "$ANIM/$5" ] && n="yes"
    printf "  %-16s %-10s %-6s %-6s %-6s %s\n" "$1" "$2" "$a" "$s" "$n" "$6"
}
spec_row "Down B Revenge"  gaogaen   speciallw      "FIGHTER_STATUS_KIND_SPECIAL_LW"  d03speciallw.nuanmb      "counter, rate<=3.0 from vl.prc"
spec_row "Side B Whip"     gaogaen   specialsstart  "FIGHTER_STATUS_KIND_SPECIAL_S"   d01specialsstart.nuanmb  "command grab, 3 follow-ups"
spec_row "Up B Fire"       gamewatch specialhi      "FIGHTER_STATUS_KIND_SPECIAL_HI"  -                        "from George; still Pikachu's animation"
echo
echo "  Not implemented:"
echo "    Neutral B  Piranha Plant Ptooie   article clone crashed on fighter load; CLONE_ENABLED=false."
echo "                                      Article id corrected to 0x6 (his own run 0x0-0x5)."
echo "    F-throw    DK cargo carry         animations extracted, carry status not built."
echo
echo "  Correct as vanilla Pikachu, per the brief:"
echo "    D-throw    unchanged by design"
