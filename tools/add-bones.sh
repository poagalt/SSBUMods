#!/usr/bin/env bash
# Adds the weapon bones a ported ACMD anchors hitboxes to but Pikachu's skeleton lacks.
#
# Chrom's jab rides `Sword1` and Palutena's n-air rides `Stick`. Both are children of `HaveR`,
# the universal item-hold bone, so they graft onto Pikachu unchanged: their bind transform is
# already expressed relative to HaveR.
#
# This replaces the old `bone-remap.sed` workaround, which rewrote both to `haver`. That was
# lossy in two ways that the drop-in architecture (ADR 0003) makes unnecessary:
#   - `Sword1` sits at HaveR's origin but carries a large bind ROTATION. Chrom's jab hitboxes are
#     offset (0,0,1.7) and (0,0,8.4) along the sword's own axis, so dropping the rotation aimed an
#     8.4-unit offset in the wrong direction entirely.
#   - `Stick` sits at [0,-2.2,0] from HaveR, so the remap also lost that offset. Palutena's n-air
#     hitboxes are at (0,+/-5.2,0) along the staff -- the observed "one missing hit".
# Both are animated in their source animation, so once the bone exists the dropped-in animation
# drives it and the hitbox is exact.
#
# Idempotent: a bone already present is left alone. Operates on the shipped model.nusktb, so run
# it after tools/merge-sword.sh.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
SKEL="$ROOT/pikachu-moveset/mod/fighter/pikachu/model/body/c00/model.nusktb"
W="$ROOT/work/bones"
mkdir -p "$W"

[ -f "$SKEL" ] || { echo "no shipped nusktb at $SKEL" >&2; exit 1; }
"$T" "$SKEL" "$W/pika.json" >/dev/null

# bone | source skeleton json | why
#
# EMPTY, BECAUSE NO HITBOX RIDES A GRAFTED BONE ANY MORE. Chrom's jab is rebased onto `haver` by
# `tools/reach/src/bin/rebase.rs` -- exact for him, measured 0.00, because `Sword1` does not move
# relative to the hand across his three live frames -- and Palutena's n-air rides the vanilla
# `HaveL` that the block at the bottom of this file reparents into `Stick`'s frame.
#
# The playtest is what decided it, and it separates the two cases cleanly: "the hitbox on cloud
# back air is fine both directions (unlike the other swords)". Cloud's ride `haver` and `top`, both
# vanilla; Chrom's rode `sword1` and Palutena's `stick`, both grafted. A bone Pikachu owns behaves
# and an appended one does not.
#
# Leaving the list here rather than deleting the mechanism: a future port whose hitboxes ride a
# bone that genuinely cannot be rebased -- one that moves relative to the hand across its live
# window, the way Palutena's staff does -- would need either a row here or the `realise` treatment,
# and the second is now the better answer.
WANT=""

cp "$W/pika.json" "$W/out.json"
added=0
while IFS='|' read -r bone src why; do
    bone="${bone// }"; src="${src// }"; why="$(echo "$why" | sed 's/^ *//;s/ *$//')"
    [ -z "$bone" ] && continue

    if jq -e --arg b "$bone" '.bones[] | select(.name == $b)' "$W/out.json" >/dev/null 2>&1; then
        echo "  $bone already present, skipped"
        continue
    fi
    # The source's parent MUST be HaveR for a verbatim graft; refuse rather than misplace it.
    pname=$(jq -r --arg b "$bone" '(.bones|to_entries) as $bs
        | $bs[] | select(.value.name==$b) | $bs[.value.parent_index].value.name' "$src")
    if [ "$pname" != "HaveR" ]; then
        echo "  REFUSING $bone: parent in source is '$pname', not HaveR -- bind transform would be wrong" >&2
        exit 1
    fi
    hidx=$(jq '[.bones|to_entries[]|select(.value.name=="HaveR")][0].key' "$W/out.json")
    jq --argjson p "$hidx" --arg b "$bone" --slurpfile s "$src" \
      '.bones += [ ($s[0].bones[] | select(.name == $b) | .parent_index = $p) ]' \
      "$W/out.json" > "$W/next.json" && mv "$W/next.json" "$W/out.json"
    echo "  + $bone  (parent HaveR = index $hidx) -- $why"
    added=$((added+1))
done <<< "$WANT"

# ==== REPURPOSE `HaveL` AS PALUTENA'S `Stick`, BECAUSE A GRAFTED BONE IS NOT DRIVEN AT RUNTIME ====
#
# Everything above grafts bones and it is now known not to be enough. `telemetry::mirror_probe`
# logs two controls: `handr`, a vanilla bone, reads IDENTICALLY in both facings on every frame --
# so the pose is the same and the probe is sound -- while `stick` and `sword` differ between
# facings in all three components including Y, which no facing can change. And `sword2`, a bone
# the animation drives and this mod never grafts, returns the exact negation of `haver_from_top`,
# which is the fighter origin: a missing bone does not fail, it returns the origin, and appended
# bones read like memory past the end of an array sized for Pikachu's vanilla bone count.
#
# Playtest, after the meshes were baked onto `HaveR`: "the facing left problem remains for all
# three attacks" and "palutena nair isn't animated correctly and has wrong hitbox".
#
# Four weapons stopped needing a grafted bone when `merge-weapons.sh` began baking them. Palutena
# cannot be baked: her HAND is nearly static from frame 6 (x 2.7-3.4, y 15-15.9 in the probe) while
# the staff traces a full circle, so the sweep is entirely `Stick`'s own 65-keyframe track.
#
# So give the track a bone the runtime owns. `HaveL` is reparented under `HaveR` and given Stick's
# bind transform, which makes it occupy Stick's frame exactly; `gen-stick-track.sh` renames the
# shipped animation's `Stick` node onto it; `merge-weapons.sh` binds the staff mesh there via the
# table's `realise` column; and `bone-remap.sed` repoints the hitboxes, with NO transform, because
# the frames coincide by construction. That last part is why the remap is lossless now -- it was
# emptied for being lossy, and it was lossy only because it mapped to `haver` and threw away
# Stick's [0,-2.2,0].
#
# WHY `HaveL` IS SAFE, checked rather than assumed: nothing in the shipped model is weighted to it
# (nor to `HaveR`), it is a LEAF with no children, and its only job is holding a left-hand item.
# The cost is that an item in the left hand would follow the right hand instead. This moveset never
# puts one there, and no mesh moving means the change is invisible otherwise.
STICKSRC="$ROOT/work/fk/palutena_skel.json"
if [ -f "$STICKSRC" ]; then
    hidx=$(jq '[.bones|to_entries[]|select(.value.name=="HaveR")][0].key' "$W/out.json")
    before=$(jq -r '(.bones|to_entries) as $bs | $bs[] | select(.value.name=="HaveL")
                    | $bs[.value.parent_index].value.name' "$W/out.json")
    jq --argjson p "$hidx" --slurpfile s "$STICKSRC" '
      ( [ $s[0].bones[] | select(.name == "Stick") ][0].transform ) as $st
      | .bones = [ .bones[]
          | if .name == "HaveL" then (.parent_index = $p | .transform = $st) else . end ]
    ' "$W/out.json" > "$W/next.json" && mv "$W/next.json" "$W/out.json"
    after=$(jq -r '(.bones|to_entries) as $bs | $bs[] | select(.value.name=="HaveL")
                   | $bs[.value.parent_index].value.name' "$W/out.json")
    echo "  ~ HaveL reparented $before -> $after, bind transform taken from palutena Stick"
    added=$((added+1))
else
    echo "  !! no $STICKSRC -- HaveL NOT repurposed, Palutena's staff will ride a grafted bone" >&2
fi

if [ "$added" -gt 0 ]; then
    "$T" "$W/out.json" "$SKEL" >/dev/null
    echo "add-bones: $added change(s), skeleton now $(jq '.bones|length' "$W/out.json") bones"
else
    echo "add-bones: nothing to do ($(jq '.bones|length' "$W/out.json") bones)"
fi
