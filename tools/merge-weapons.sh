#!/usr/bin/env bash
# Merges every source fighter's weapon into Pikachu's body model as toggleable meshes.
#
# GENERALISES tools/merge-sword.sh, which did Chrom's Falchion and nothing else. The technique is
# unchanged -- LilyLavender Modding Workshop episode 39: give the mesh object a distinct name so
# `ModelModule::set_mesh_visibility` can target it, single-bind it to a bone grafted under
# Pikachu's own skeleton, and declare the textures via new-dir-files rather than shipping them.
#
# WHY THIS WORKS AT ALL, AND WHY IT IS NOT `clone_weapon`. A real article needs the game's own
# article initialisation, and `smashline::clone_weapon` crashes inside
# `fighter_initialize_object_data` -- the Ptooie blocker in handbook/open-questions.md. A merged
# mesh needs none of that: it is part of Pikachu's model, it inherits his skeleton's animation, and
# a visibility flag is the whole of its lifecycle.
#
# WHAT IT CANNOT DO, stated up front because it decides which weapons are in this table.
#
# The graft carries ONE bone. That is exact for a RIGID weapon -- a sword is a stick, its bone is
# the handle, and Pikachu's hand animation moves it correctly for free. It is not exact for an
# article whose own skeleton ANIMATES: Incineroar's rope bends across RopeC/RopeL1..3/RopeR1..3,
# Game & Watch's parachute across Parachutea1/a2/b1/b2, Corrin's dragon hand across Hand1..4. Those
# shapes come from the article's own `model.nuanmb`, which nothing here plays, so a graft would
# show them as rigid geometry. They are deliberately absent from this table; see
# docs/the-weapons.md for what they would need instead.
#
# EVERY NUMBER BELOW IS THE GAME'S OWN. The bone parent, the material label, the mesh object name
# and the bounding sphere are all read out of the source fighter's own files -- the spheres in
# particular come from each article's `model.numshexb`, so culling behaves as the game intends
# rather than as a radius someone guessed. `merge-sword.sh` computed Chrom's by hand from the
# blade's vertices and got (0,0,4.5) r=7; that row keeps those values so this tool reproduces the
# shipped model byte-for-byte on the Chrom entry alone.
#
# Regenerate with:  bash tools/merge-weapons.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
PIK="$ROOT/extracted/fighter/pikachu/model/body/c00"
OUT="$ROOT/pikachu-moveset/mod/fighter/pikachu/model/body/c00"
WORK="$ROOT/work/weapons"

# mesh_name | source model dir | source mesh object | material | bone | parent | cx,cy,cz | radius
#
# `source model dir` is relative to a fighter root and searched in BOTH extracted/ and
# work/arc-weapons/, because the four weapons past Chrom were pulled from the running game by
# `tools/arc-fetch.sh` and only Chrom's was in the original extraction.
TABLE="
chrom_sword|chrom/model/sword|fal_RShape|pasted__met_chrom_001|Sword|HaveR|0,0,4.5|7.0
cloud_sword|cloud/model/fusionsword|fusionsword1_VIS_O_OBJShape|def_cloud_004|fusionsword1|HaveR|0,6.26053,0|10.0666
mythra_sword|elight/model/esword|lightsword_VIS_O_OBJShape|alp_light_002|Sword1|HaveR|0,0,4.29851|6.57543
palutena_staff|palutena/model/body|staff_group_VIS_O_OBJShape|metal_palutena_002_a|Stick|HaveR|0.0275341,-0.65733,0.0205462|10.1638
"

rm -rf "$WORK"; mkdir -p "$WORK" "$OUT"
cd "$WORK"

# Pikachu's five files, as JSON, once. Every weapon appends to these in turn.
for f in numshb numdlb numatb nusktb numshexb; do
    "$T" "$PIK/model.$f" "cur_$f.json" >/dev/null
done
cp "$PIK/model.adjb" "$OUT/model.adjb"

base_bones=$(jq '.bones | length' cur_nusktb.json)
base_meshes=$(jq '.objects | length' cur_numshb.json)

printf '%-16s %-9s %-7s %s\n' WEAPON BONE MESH SOURCE
while IFS='|' read -r name dir mesh material bone parent sphere radius; do
    case "${name:-}" in '' | \#*) continue ;; esac

    src=""
    for root in "$ROOT/extracted/fighter" "$ROOT/work/arc-weapons/fighter"; do
        [ -f "$root/$dir/c00/model.numshb" ] && src="$root/$dir/c00" && break
    done
    [ -n "$src" ] || { echo "MISSING source model for $name ($dir)" >&2; exit 1; }

    for f in numshb numdlb numatb nusktb numshexb; do
        "$T" "$src/model.$f" "src_$f.json" >/dev/null
    done

    # THE PARENT IS LOOKED UP BY NAME, NOT ASSUMED. Every weapon in this table hangs its bone off
    # `HaveR`, and so does Pikachu -- but a future row with a different anchor must not silently
    # graft onto bone 0.
    pidx=$(jq --arg p "$parent" '[.bones | to_entries[] | select(.value.name == $p)][0].key' cur_nusktb.json)
    [ "$pidx" != "null" ] || { echo "$parent not found in Pikachu's skeleton" >&2; exit 1; }

    # A MESH NAME COLLISION WOULD SILENTLY REBIND SOMETHING OF PIKACHU'S. Checked, not hoped for.
    # Bone collisions cannot happen at all now: every grafted bone is prefixed, see below.
    if jq -e --arg n "$name" '[.objects[] | .name] | index($n)' cur_numshb.json >/dev/null 2>&1; then
        echo "name collision: $name already exists in Pikachu's .objects" >&2; exit 1
    fi

    # EVERY GRAFTED BONE IS PREFIXED WITH THE WEAPON'S NAME, and that is not cosmetic.
    #
    # `tools/add-bones.sh` ALSO puts bones in this skeleton, from its own hand-kept list, for a
    # different reason: a ported ACMD anchors hitboxes to a source bone name, so Chrom's jab needs
    # a `Sword1` and Palutena's n-air needs a `Stick`. It takes `Sword1` from CHROM's skeleton.
    #
    # This tool used to graft a `Sword1` too -- MYTHRA's, the bone her sword mesh is bound to.
    # Same name, completely different bone:
    #
    #     chrom  Sword1  [[-0.18,-0.49,-0.85,0],[-0.98,0.11,0.15,0],[0.01,0.86,-0.51,0],...]
    #     elight Sword1  [[  0  ,  1   ,  0  ,0],[ -1 ,  0  ,  0  ,0],[  0 ,  0  ,  1  ,0],...]
    #
    # This tool runs FIRST and `add-bones.sh` is idempotent -- "a bone already present is left
    # alone" -- so the merge silently won and Chrom's jab hitboxes, documented as riding the blade
    # at (0,0,1.7) and (0,0,8.4), were aimed with Mythra's bind rotation. Two tools maintaining
    # overlapping lists from different source skeletons, which is the exact shape
    # `tools/check-lists.sh` exists to prevent.
    #
    # Prefixing removes the class of bug rather than this instance of it: a graft can no longer
    # shadow any bone Pikachu has or any bone another tool provides, whatever order they run in.
    jq --argjson anchor "$pidx" --arg b "$bone" --arg pfx "${name}_" \
       --slurpfile sk src_nusktb.json '
      .bones += [ ( $sk[0].bones[] | select(.name == $b)
                    | .name = ($pfx + $b)
                    | .parent_index = $anchor ) ]
    ' cur_nusktb.json > next_nusktb.json

    # ONE BONE, AND THE MESH IS FORCED RIGID ONTO IT. That is the documented technique -- episode
    # 39 says "single-bind it to a bone grafted under Pikachu's own skeleton" -- and departing from
    # it is what cost four hung boots.
    #
    # A GRAFTED BONE EXISTS IN THE FILE AND NOT AT RUNTIME. The header of
    # `tools/reach/src/bin/rebase.rs` proves it with `telemetry::mirror_probe`: `Sword1` and
    # `Stick` read differently between facings and nowhere near their parent `HaveR`, so they are
    # not in the runtime transform chain. A RIGID mesh survives that -- Chrom's Falchion rendered
    # for weeks on exactly this graft. A SKINNED one does not have the same luck: it needs a
    # per-vertex index into a runtime matrix palette that has no entry for those bones.
    #
    # Mythra's `lightsword_VIS_O_OBJShape` is the only skinned source mesh of the four, weighted to
    # Sword1 plus the two blade-glow bones. Grafting all three bones so the weights resolved ON
    # DISK did not help, because the problem was never on disk. Stripping the weights and
    # single-binding is what the technique actually asks for.
    #
    # SUBINDEX 0 ONLY. Several of these meshes appear more than once under one name -- Mythra's has
    # four and Palutena's staff two, which are the game's own LOD and variant slots. Taking the
    # first is what `set_mesh_visibility` can address by name and is the highest-detail one.
    jq --arg n "$name" --arg m "$mesh" --arg pb "${name}_${bone}" --slurpfile s src_numshb.json '
      .objects += [ ( [ $s[0].objects[] | select(.name == $m) ][0]
                      | .name = $n
                      | .subindex = 0
                      | .parent_bone_name = $pb
                      | .bone_influences = [] ) ]
    ' cur_numshb.json > next_numshb.json
    jq --arg n "$name" --arg m "$material" '
      .entries += [{ mesh_object_name: $n, mesh_object_subindex: 0, material_label: $m }]
    ' cur_numdlb.json > next_numdlb.json

    # The material is appended only if Pikachu does not already carry that label -- two weapons
    # from one fighter would otherwise duplicate it and the game rejects duplicate labels.
    jq --arg m "$material" --slurpfile s src_numatb.json '
      if ([.entries[] | .material_label] | index($m)) then .
      else .entries += [ ( $s[0].entries[] | select(.material_label == $m) ) ] end
    ' cur_numatb.json > next_numatb.json

    cx=${sphere%%,*}; rest=${sphere#*,}; cy=${rest%%,*}; cz=${rest##*,}
    jq --arg n "$name" --argjson cx "$cx" --argjson cy "$cy" --argjson cz "$cz" \
       --argjson r "$radius" '
      .mesh_object_groups += [{
        bounding_sphere: { center: { x: $cx, y: $cy, z: $cz }, radius: $r },
        mesh_object_full_name: $n,
        mesh_object_name: $n,
        entry_flags: [ { draw_model: true, cast_shadow: true } ]
      }]
    ' cur_numshexb.json > next_numshexb.json

    for f in numshb numdlb numatb nusktb numshexb; do mv "next_$f.json" "cur_$f.json"; done
    printf '%-16s %-9s %-7s %s\n' "$name" "$bone" "ok" "${src#$ROOT/}"
done <<< "$TABLE"

for f in numshb numdlb numatb nusktb numshexb; do
    "$T" "cur_$f.json" "$OUT/model.$f" >/dev/null
done

echo
echo "skeleton: $(jq '.bones | length' cur_nusktb.json) bones (Pikachu alone has $base_bones)"
echo "mesh:     $(jq '.objects | length' cur_numshb.json) objects (Pikachu alone has $base_meshes)"
echo "matl:     $(jq '.entries | length' cur_numatb.json) materials"
echo "meshex:   $(jq '.mesh_object_groups | length' cur_numshexb.json) groups"
echo
echo "textures are NOT shipped: config.json declares the paths and share-to-added points them at"
echo "each source fighter's vanilla textures, so no Nintendo texture data is redistributed."
