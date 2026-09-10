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
# against e.g. Mythra's 1.112. A raw drop-in therefore lands at 0.96/1.112 = 86% of her world
# reach -- measured as 0.52-2.19 world units of error across the ported moves, correlating
# exactly with each fighter's ratio, and reported in playtest as Fox's u-smash lacking horizontal
# range and Little Mac's f-smash arm being too short.
#
# The correction multiplies the `Rot` node by ratio = source_scale / 0.96. `Rot` is the root of
# the posed skeleton (chain: Trans > Rot > Hip > ...), and `compensate_scale` is false throughout
# these animations, so a scale there inherits down every bone. Because a hitbox's bone-local
# offset is transformed by the bone's world matrix, that inherited scale corrects the offsets too
# -- one node instead of ~100 tracks plus an ACMD pass. `Rot`'s own translation is the standing
# height and is scaled with it.
#
# `Trans` is deliberately untouched: its translation is root motion, which the game consumes to
# move the fighter (Little Mac's f-smash runs 0 -> 17.77 there). Whether the engine applies the
# fighter's scale to root motion is not something the animation data can answer, so it is left
# alone and measured on hardware instead -- telemetry logs `moved x` per motion.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/extracted/fighter"
DST="$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/c00"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
W="$ROOT/work/anim"
PIKA_SCALE=0.96
mkdir -p "$DST" "$W" "$ROOT/work/fk"

# NEUTRALISED BONES
# A bone Pikachu has but the source does not gets no track from the dropped-in animation, so his
# own bind transform stays in the chain and inserts an offset the source never had. Giving it an
# identity track removes it, making the chain match the source's exactly. tools/chain-audit.sh
# finds these; today the only one is Fox's u-smash, where Pikachu's leg runs
# Hip > LegC > LegR while Fox's runs Hip > LegR (worth 0.46-0.69 world units).
#
# source fighter | animation file | source `scale` param | neutralise | move it drives
MOVES="
chrom     | c00attack11.nuanmb    | 0.96  | -    | jab (Chrom)
elight    | c00attackdash.nuanmb  | 1.112 | -    | dash attack (Mythra)
elight    | c01attacks3s.nuanmb   | 1.112 | -    | f-tilt (Mythra)
falco     | c02attackhi3.nuanmb   | 1.05  | -    | u-tilt (Falco)
littlemac | c03attacks4s.nuanmb   | 1.11  | -    | f-smash (Little Mac) -- lunge lives in Trans
fox       | c04attackhi4.nuanmb   | 1.08  | LegC | u-smash (Fox)
palutena  | c05attackairn.nuanmb  | 0.95  | -    | n-air (Palutena)
demon     | c05attackairf.nuanmb  | 1.007 | -    | f-air (Kazuya)
cloud     | c05attackairb.nuanmb  | 1.03  | -    | b-air (Cloud)
kamui     | c05attackairhi.nuanmb | 0.99  | -    | u-air (Corrin)
krool     | c05attackairlw.nuanmb | 1.15  | -    | d-air (K. Rool)
"

n=0; miss=0; scaled=0; neutralised=0
while IFS='|' read -r fighter anim sscale neut label; do
    fighter="${fighter// }"; anim="${anim// }"; sscale="${sscale// }"; neut="${neut// }"
    label="$(echo "$label" | sed 's/^ *//;s/ *$//')"
    [ -z "$fighter" ] && continue
    s="$SRC/$fighter/motion/body/c00/$anim"
    if [ ! -f "$s" ]; then
        echo "  MISSING  $fighter/$anim -- $label" >&2
        miss=$((miss+1)); continue
    fi
    stem="${anim%.nuanmb}"
    ratio=$(awk -v a="$sscale" -v b="$PIKA_SCALE" 'BEGIN{printf "%.6f", a/b}')

    if awk -v r="$ratio" 'BEGIN{exit !(r > 0.9995 && r < 1.0005)}' && [ "$neut" = "-" ]; then
        # Same scale as Pikachu and nothing to neutralise; a copy is already exact.
        cp "$s" "$DST/$anim"
        printf "  %-22s <- %-10s ratio %s (exact, copied)  %s\n" "$anim" "$fighter" "$ratio" "$label"
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

        # Refuse to ship an animation whose Rot node was not actually rewritten.
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

        # Neutralise Pikachu-only chain bones by giving them an identity track, cloned from an
        # existing node so the track's shape (flags, frame count) stays valid.
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
        printf "  %-22s <- %-10s ratio %s (Rot %s -> %s)  %s\n" "$anim" "$fighter" "$ratio" "$before" "$after" "$label"
        scaled=$((scaled+1))
    fi
    n=$((n+1))
done <<< "$MOVES"

echo "anim-dropin: $n shipped ($scaled scale-compensated, $neutralised bones neutralised), $miss missing"
[ "$miss" -eq 0 ] || exit 1
