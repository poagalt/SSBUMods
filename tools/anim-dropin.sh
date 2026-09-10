#!/usr/bin/env bash
# Ships each source fighter's own animation in Pikachu's motion directory, so a ported move's
# verbatim bone-anchored hitboxes land exactly where the source's do. See ADR 0003.
#
# SSBU names an animation the same on every fighter -- motion `attack_s3_s` is always
# `c01attacks3s.nuanmb` -- so this is a same-path replacement. No motion_list or config.json
# change is needed, and tools/motion-patch/target/release/dropin.exe pre-flights the flags.
#
# A move listed here MUST have its ACMD ported with the source's original bone anchors intact.
# Re-anchoring a hitbox to `top` on top of a dropped-in animation would double-compensate.
#
# SCALE COMPENSATION
# The game multiplies a fighter's whole skeleton by its `scale` param, and Pikachu's is 0.96
# against e.g. Mythra's 1.112. A raw drop-in therefore lands short of the source's world reach --
# reported in playtest as Fox's u-smash lacking horizontal range and Little Mac's f-smash arm
# being too short.
#
# The correction multiplies the `Rot` node by ratio = source_scale / 0.96. `Rot` is the root of
# the posed skeleton (chain: Trans > Rot > Hip > ...), and `compensate_scale` is false throughout
# these animations, so a scale there inherits down every bone. Because a hitbox's bone-local
# offset is transformed by the bone's world matrix, that inherited scale corrects the offsets too
# -- one node instead of ~100 tracks. `Rot`'s own translation is the standing height and is
# scaled with it.
#
# Scales come from tools/scales.tsv, generated from the game's own fighter_param.prc. They used to
# be hand-entered here and two were wrong (Cloud 1.03 vs 0.98, K. Rool 1.15 vs 0.89 -- a 29%
# over-scale on his d-air) and the hitbox trace could not see it, because it applied the same
# wrong constant to both sides of its comparison.
#
# `Trans` is deliberately untouched: its translation is root motion, which the game consumes to
# move the fighter. Measured on hardware -- Little Mac's f-smash travels +20.59 world units
# against his own 19.72, within 4.4% -- so no correction is applied there.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/extracted/fighter"
DST="$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/c00"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
W="$ROOT/work/anim"
SCALES="$ROOT/tools/scales.tsv"
mkdir -p "$DST" "$W" "$ROOT/work/fk"
[ -f "$SCALES" ] || { echo "no tools/scales.tsv -- run tools/gen-scales.sh" >&2; exit 1; }

PIKA_SCALE=$(awk -F'\t' '$1=="pikachu"{print $3}' "$SCALES")
[ -n "$PIKA_SCALE" ] || { echo "pikachu missing from scales.tsv" >&2; exit 1; }

scale_of() {
    local s
    s=$(awk -F'\t' -v f="$1" '$1==f{print $3}' "$SCALES")
    [ -n "$s" ] || { echo "no scale for fighter '$1' in scales.tsv" >&2; exit 1; }
    echo "$s"
}

# NEUTRALISED BONES
# A bone Pikachu has but the source does not gets no track from the dropped-in animation, so his
# own bind transform stays in the chain and inserts an offset the source never had. Giving it an
# identity track removes it. tools/chain-audit.sh finds these.
#
# Little Mac's ANGLED f-smash animations are filenames Pikachu does not have, so they are NEW
# paths: they need a config.json new-dir-files entry AND tools/motion-patch synth must point
# attack_s4_hi/attack_s4_lw at them (adopt_anim). Without both they simply do not load.
#
# source fighter | animation file | neutralise | move it drives
MOVES="
chrom     | c00attack11.nuanmb        | -    | jab
elight    | c00attackdash.nuanmb      | -    | dash attack
elight    | c01attacks3s.nuanmb       | -    | f-tilt (all three angles; she has no angled animations)
falco     | c02attackhi3.nuanmb       | -    | u-tilt
robot     | c02attacklw3.nuanmb       | -    | d-tilt
littlemac | c03attacks4s.nuanmb       | -    | f-smash, straight
littlemac | c03attacks4hi.nuanmb      | -    | f-smash, up-angled (new file: see config.json new-dir-files)
littlemac | c03attacks4lw.nuanmb      | -    | f-smash, down-angled (new file: see config.json new-dir-files)
littlemac | c03attacks4charge.nuanmb  | -    | f-smash charge hold
fox       | c04attackhi4.nuanmb       | LegC | u-smash
fox       | c04attackhi4charge.nuanmb | -    | u-smash charge hold
gamewatch | c04attacklw4.nuanmb       | -    | d-smash
gamewatch | c04attacklw4charge.nuanmb | -    | d-smash charge hold
palutena  | c05attackairn.nuanmb      | -    | n-air
demon     | c05attackairf.nuanmb      | -    | f-air
cloud     | c05attackairb.nuanmb      | -    | b-air
kamui     | c05attackairhi.nuanmb     | -    | u-air
krool     | c05attackairlw.nuanmb     | -    | d-air
palutena  | c05landingairn.nuanmb     | -    | n-air landing
demon     | c05landingairf.nuanmb     | -    | f-air landing
cloud     | c05landingairb.nuanmb     | -    | b-air landing
kamui     | c05landingairhi.nuanmb    | -    | u-air landing
krool     | c05landingairlw.nuanmb    | -    | d-air landing
palutena  | e00catch.nuanmb           | -    | grab
palutena  | e00catchdash.nuanmb       | -    | dash grab
palutena  | e00catchturn.nuanmb       | -    | pivot grab
palutena  | e00catchwait.nuanmb       | -    | grab hold
palutena  | e00catchpull.nuanmb       | -    | grab pull
palutena  | e00catchcut.nuanmb        | -    | grab break
demon     | e00catchattack.nuanmb     | -    | pummel
gaogaen   | e01throwb.nuanmb          | -    | b-throw
mewtwo    | e01throwhi.nuanmb         | -    | u-throw
koopa     | f03downattacku.nuanmb     | -    | get-up attack, face up
koopa     | f04downattackd.nuanmb     | -    | get-up attack, face down
koopa     | g03slipattack.nuanmb      | -    | trip attack
ganon     | g02cliffattack.nuanmb     | -    | ledge attack
fox       | b02escapeair.nuanmb       | -    | air dodge (and directional, which shares it)
fox       | b02escapef.nuanmb         | -    | forward roll
"

n=0; miss=0; scaled=0; neutralised=0
while IFS='|' read -r fighter anim neut label; do
    fighter="${fighter// }"; anim="${anim// }"; neut="${neut// }"
    label="$(echo "$label" | sed 's/^ *//;s/ *$//')"
    [ -z "$fighter" ] && continue
    s="$SRC/$fighter/motion/body/c00/$anim"
    if [ ! -f "$s" ]; then
        echo "  MISSING  $fighter/$anim -- $label" >&2
        miss=$((miss+1)); continue
    fi
    stem="${anim%.nuanmb}"
    sscale=$(scale_of "$fighter")
    ratio=$(awk -v a="$sscale" -v b="$PIKA_SCALE" 'BEGIN{printf "%.6f", a/b}')

    if awk -v r="$ratio" 'BEGIN{exit !(r > 0.9995 && r < 1.0005)}' && [ "$neut" = "-" ]; then
        cp "$s" "$DST/$anim"
        printf "  %-26s <- %-10s ratio %s  %s\n" "$anim" "$fighter" "$ratio" "$label"
    else
        "$T" "$s" "$W/$stem.in.json" >/dev/null
        jq --argjson r "$ratio" '
          .groups |= map(
            if .group_type == "Transform" then
              .nodes |= map(
                if (.name | ascii_downcase) == "rot" then
                  .tracks |= map(
                    .values.Transform |= map(
                      .scale.x *= $r | .scale.y *= $r | .scale.z *= $r
                    | .translation.x *= $r | .translation.y *= $r | .translation.z *= $r
                    )
                  )
                else . end
              )
            else . end
          )' "$W/$stem.in.json" > "$W/$stem.out.json"

        rotx() { jq -r '[.groups[]|select(.group_type=="Transform")|.nodes[]|select((.name|ascii_downcase)=="rot")|.tracks[0].values.Transform[0].scale.x]|.[0] // "none"' "$1"; }
        before=$(rotx "$W/$stem.in.json"); after=$(rotx "$W/$stem.out.json")
        if [ "$before" = "none" ]; then
            echo "  REFUSING $anim: no Rot node to scale -- would ship uncompensated" >&2
            exit 1
        fi
        if awk -v r="$ratio" 'BEGIN{exit !(r < 0.9995 || r > 1.0005)}' && [ "$before" = "$after" ]; then
            echo "  REFUSING $anim: Rot scale unchanged ($before) -- the jq filter did not apply" >&2
            exit 1
        fi

        if [ "$neut" != "-" ]; then
            for b in $(echo "$neut" | tr ',' ' '); do
                jq --arg b "$b" '
                  ([.groups[] | select(.group_type=="Transform") | .nodes[0]] | .[0]) as $tpl
                  | ($tpl.tracks[0].values.Transform | length) as $nf
                  | .groups |= map(
                      if .group_type == "Transform" then
                        .nodes |= ( map(select((.name|ascii_downcase) != ($b|ascii_downcase)))
                                    + [ $tpl
                                        | .name = $b
                                        | .tracks[0].values.Transform =
                                            [ range($nf) | {
                                                scale:       {x:1.0, y:1.0, z:1.0},
                                                rotation:    {x:0.0, y:0.0, z:0.0, w:1.0},
                                                translation: {x:0.0, y:0.0, z:0.0}
                                              } ] ] )
                      else . end )' "$W/$stem.out.json" > "$W/$stem.neut.json"
                got=$(jq -r --arg b "$b" '[.groups[]|select(.group_type=="Transform")|.nodes[]|select((.name|ascii_downcase)==($b|ascii_downcase))]|length' "$W/$stem.neut.json")
                if [ "$got" != "1" ]; then
                    echo "  REFUSING $anim: neutralising $b produced $got nodes, expected 1" >&2
                    exit 1
                fi
                mv "$W/$stem.neut.json" "$W/$stem.out.json"
                neutralised=$((neutralised+1))
            done
        fi
        "$T" "$W/$stem.out.json" "$DST/$anim" >/dev/null
        # Keep a dump of exactly what we ship, so tools/trace-all.sh measures the real thing.
        cp "$W/$stem.out.json" "$ROOT/work/fk/shipped_$stem.json"
        printf "  %-26s <- %-10s ratio %s (Rot %s -> %s)%s  %s\n" "$anim" "$fighter" "$ratio" \
               "$before" "$after" "$([ "$neut" != "-" ] && echo " +neut $neut")" "$label"
        scaled=$((scaled+1))
    fi
    n=$((n+1))
done <<< "$MOVES"

echo "anim-dropin: $n shipped ($scaled scale-compensated, $neutralised bones neutralised), $miss missing"
[ "$miss" -eq 0 ] || exit 1
