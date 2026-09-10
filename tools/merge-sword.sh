#!/usr/bin/env bash
# Merges Chrom's Falchion into Pikachu's body model as a toggleable mesh.
#
# Technique is LilyLavender Modding Workshop episode 39: a distinct mesh-object name so
# ModelModule::set_mesh_visibility can target it, single-bound to a bone, textures declared
# via new-dir-files.
#
# Chrom's sword article has skeleton HaveR -> Sword where Sword's local transform is the
# transpose of HaveR's rotation, i.e. the blade sits at identity in article space. Rather
# than rebasing ~1600 vertices, we add a `Sword` bone under Pikachu's HaveR carrying that
# same local transform and leave the mesh vertices untouched.
#
# NOTE: the resulting world orientation is Pikachu's HaveR rotation composed with Chrom's
# inverse hand rotation, which is not guaranteed to look right. SWORD_TRANSFORM below is the
# single knob to calibrate once it can be seen in game.
#
# Regenerate with:  bash tools/merge-sword.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
SRC="$ROOT/extracted/fighter/chrom/model/sword/c00"
PIK="$ROOT/extracted/fighter/pikachu/model/body/c00"
WORK="$ROOT/work/sword"
OUT="$ROOT/pikachu-moveset/mod/fighter/pikachu/model/body/c00"

MESH_NAME="chrom_sword"
MATERIAL="pasted__met_chrom_001"

mkdir -p "$WORK" "$OUT"
cd "$WORK"

for f in numshb numdlb numatb nusktb; do
    "$T" "$PIK/model.$f"  "pika_$f.json"  >/dev/null
    "$T" "$SRC/model.$f"  "sword_$f.json" >/dev/null
done

HAVER_IDX=$(jq '[.bones | to_entries[] | select(.value.name == "HaveR")][0].key' pika_nusktb.json)
if [ "$HAVER_IDX" = "null" ]; then echo "HaveR not found in Pikachu's skeleton" >&2; exit 1; fi
echo "HaveR is bone index $HAVER_IDX of $(jq '.bones | length' pika_nusktb.json)"

# 1. skeleton: append a Sword bone under HaveR, carrying the article's local transform
jq --argjson p "$HAVER_IDX" --slurpfile sword sword_nusktb.json '
  .bones += [ ( $sword[0].bones[] | select(.name == "Sword") | .parent_index = $p ) ]
' pika_nusktb.json > merged_nusktb.json
echo "skeleton: $(jq '.bones | length' merged_nusktb.json) bones (was $(jq '.bones | length' pika_nusktb.json))"

# 2. mesh: append the blade, renamed so set_mesh_visibility can target it
jq --arg n "$MESH_NAME" '
  .objects += [ ( $sword[0].objects[] | select(.name == "fal_RShape") | .name = $n ) ]
' --slurpfile sword sword_numshb.json pika_numshb.json > merged_numshb.json
echo "mesh: $(jq '.objects | length' merged_numshb.json) objects (was $(jq '.objects | length' pika_numshb.json))"

# 3. modl: bind the new mesh to the sword material
jq --arg n "$MESH_NAME" --arg m "$MATERIAL" '
  .entries += [{ mesh_object_name: $n, mesh_object_subindex: 0, material_label: $m }]
' pika_numdlb.json > merged_numdlb.json
echo "modl: $(jq '.entries | length' merged_numdlb.json) entries"

# 4. matl: append the sword material
jq --arg m "$MATERIAL" '
  .entries += [ ( $sword[0].entries[] | select(.material_label == $m) ) ]
' --slurpfile sword sword_numatb.json pika_numatb.json > merged_numatb.json
echo "matl: $(jq '.entries | length' merged_numatb.json) materials"

# 5. back to binary
"$T" merged_nusktb.json "$OUT/model.nusktb" >/dev/null
"$T" merged_numshb.json "$OUT/model.numshb" >/dev/null
"$T" merged_numdlb.json "$OUT/model.numdlb" >/dev/null
"$T" merged_numatb.json "$OUT/model.numatb" >/dev/null

# 6. Textures are NOT shipped. config.json declares the three paths via new-dir-files and
#    points them at Chrom's vanilla textures via share-to-added, so no Nintendo texture
#    data is redistributed.

echo
ls -la "$OUT"

# 7. numshexb: the new mesh object needs a group entry, or it is absent from the
#    draw/shadow table. Bounding sphere computed from the blade's own vertices
#    (z spans -1.83..10.91, x +-1.40), so centre (0,0,4.5) radius 7 encloses it.
"$T" "$PIK/model.numshexb" "$WORK/pika_numshexb.json" >/dev/null
jq --arg n "$MESH_NAME" '
  .mesh_object_groups += [{
    bounding_sphere: { center: { x: 0.0, y: 0.0, z: 4.5 }, radius: 7.0 },
    mesh_object_full_name: $n,
    mesh_object_name: $n,
    entry_flags: [ { draw_model: true, cast_shadow: true } ]
  }]
' "$WORK/pika_numshexb.json" > "$WORK/merged_numshexb.json"
"$T" "$WORK/merged_numshexb.json" "$OUT/model.numshexb" >/dev/null
echo "meshex: $(jq '.mesh_object_groups | length' "$WORK/merged_numshexb.json") groups (was $(jq '.mesh_object_groups | length' "$WORK/pika_numshexb.json"))"

# 8. adjb is an adjacency buffer keyed to specific meshes; the new object is absent from it,
#    which is the same situation as any mesh without adjacency data. Shipped unmodified.
cp "$PIK/model.adjb" "$OUT/model.adjb"
