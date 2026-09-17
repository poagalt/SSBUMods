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
    hlchk_before=$(jq -r '(.bones|to_entries) as $bs | $bs[] | select(.value.name=="HaveL")
                    | $bs[.value.parent_index].value.name' "$W/out.json")
    # AND IT IS MOVED TO THE END OF THE BONE LIST, because reparenting it in place created a
    # FORWARD REFERENCE and that is very likely why the staff was still wrong facing left.
    #
    # `HaveL` is bone 17 and `HaveR` is bone 22, so "HaveL's parent is HaveR" means a bone whose
    # parent has a HIGHER index. If the engine resolves the skeleton in a single forward pass --
    # which is the usual way -- bone 17 is computed before bone 22 exists this frame, so the staff
    # would hang off a ONE-FRAME-STALE `HaveR`. A frame of lag is invisible while the hand is
    # still and catastrophic across a facing flip, where the stale matrix belongs to the OTHER
    # direction. "Works to the right, still broken to the left" is exactly that shape.
    #
    # Vanilla does contain forward references -- `UplipL1` at 41 parented to `UplipL1_offset` at
    # 55, and nine more like it -- but every one is an `_offset` helper pair, which `model.nuhlpb`
    # drives through a separate constraint mechanism. They do not establish that an ordinary bone
    # may point forward, so this does not rely on it.
    #
    # The two vanilla bones that already sit after `HaveR` and are unused leaves were both
    # rejected: `Throw` (36) is where the engine puts a thrown victim, and `Mouth` (37) anchors
    # mouth effects. Moving `HaveL` costs nothing by comparison -- nothing is skinned to it, it has
    # no children, and every reference to it anywhere is BY NAME.
    #
    # Every other bone keeps its relative order; indices above 17 shift down by one and every
    # `parent_index` is remapped with them. The check below asserts that each bone's parent still
    # resolves to the same NAME it did before the move, which is the whole correctness condition.
    jq --slurpfile s "$STICKSRC" '
      ( [ $s[0].bones[] | select(.name == "Stick") ][0].transform ) as $st
      | .bones as $old
      | ( [ $old | to_entries[] | select(.value.name == "HaveL") ][0].key ) as $li
      | ( [ $old[] | .name ] ) as $names
      # old index -> name, so the remap can be asserted by name afterwards.
      | ( [ $old | to_entries[] | select(.value.name != "HaveL") | .value ] ) as $rest
      | ( [ $old | to_entries[] | select(.value.name == "HaveL") | .value ][0] ) as $hl
      # new index of an old index: drop one if it was past the removed slot.
      # `null`, NOT `-1`, FOR THE ROOT. `parent_index` deserialises as `Option<usize>` -- UNSIGNED
      # -- so `-1` is not a representable value. Emitting it made ssbh_data_json fail to parse the
      # skeleton, fall through to another format, and panic with `missing field "entries"`, which
      # names a MODL field and points nowhere near the real cause. The write never happened and
      # `add-bones` aborted after printing its success lines, so the shipped skeleton silently kept
      # the vanilla `HaveL` the whole time. Vanilla writes `null` for `Trans`; so does this.
      | def newidx($i): if $i == null then null elif $i > $li then $i - 1 else $i end;
        ( [ $rest[] | .parent_index = newidx(.parent_index) ] ) as $fixed
      | ( [ $fixed | to_entries[] | select(.value.name == "HaveR") ][0].key ) as $hr
      | .bones = ( $fixed + [ $hl | .parent_index = $hr | .transform = $st ] )
    ' "$W/out.json" > "$W/next.json" && mv "$W/next.json" "$W/out.json"
    # THE REMAP IS ASSERTED, NOT TRUSTED: every bone's parent must still be the same NAME.
    hlchk_before=$(jq -r '.bones as $B | [ $B[] | "\(.name)<-\(if .parent_index>=0 then $B[.parent_index].name else "ROOT" end)" ] | sort | join(",")' "$W/pika.json")
    hlchk_after=$(jq -r '.bones as $B | [ $B[] | select(.name != "HaveL") | "\(.name)<-\(if .parent_index>=0 then $B[.parent_index].name else "ROOT" end)" ] | sort | join(",")' "$W/out.json")
    hlchk_ref=$(jq -r '.bones as $B | [ $B[] | select(.name != "HaveL") | "\(.name)<-\(if .parent_index>=0 then $B[.parent_index].name else "ROOT" end)" ] | sort | join(",")' "$W/pika.json")
    if [ "$hlchk_after" != "$hlchk_ref" ]; then
        echo "  REFUSING: moving HaveL changed some other bone's parent -- the index remap is wrong" >&2
        exit 1
    fi
    fwd=$(jq -r '[ .bones | to_entries[] | select(.value.parent_index != null and .value.parent_index >= .key) | .value.name ] | length' "$W/out.json")
    echo "  ok  index remap preserves every other parent; $fwd forward reference(s) remain (vanilla has 10 _offset pairs)"
    hlchk_after=$(jq -r '(.bones|to_entries) as $bs | $bs[] | select(.value.name=="HaveL")
                   | $bs[.value.parent_index].value.name' "$W/out.json")
    echo "  ~ HaveL moved to the end of the bone list, reparented onto HaveR, bind transform from palutena Stick"
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
