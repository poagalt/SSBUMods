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
    # Bones are handled in the graft loop below, which skips one Pikachu already has by name
    # rather than refusing -- a source mesh weighted to `HaveR` names a bone that is legitimately
    # the same bone on both skeletons.
    if jq -e --arg n "$name" '[.objects[] | .name] | index($n)' cur_numshb.json >/dev/null 2>&1; then
        echo "name collision: $name already exists in Pikachu's .objects" >&2; exit 1
    fi

    # WHICH BONES THE MESH ACTUALLY NEEDS, READ OUT OF THE MESH RATHER THAN NAMED IN THE TABLE.
    #
    # THIS COST A WHOLE AFTERNOON OF PLAYTESTS. Three of these weapons are RIGID: `bone_influences`
    # is empty and the mesh hangs off a single bone, so one graft is exact and Chrom's sword worked
    # for weeks. Mythra's is SKINNED -- `lightsword_VIS_O_OBJShape` is weighted to Sword1,
    # Sword_lightC and Sword_lightD -- and grafting only the table's bone left two of its three
    # influences naming bones that do not exist in Pikachu's skeleton.
    #
    # Vanilla Pikachu has ZERO influence bones absent from his own nusktb; the shipped merged model
    # had exactly those two. The training-mode load then hung FOREVER on loading him -- not a
    # crash: no crash report, ARCropolis finished its filesystem work and went quiet, the plugin
    # installed in full, the loading animation kept spinning, and no fighter object was ever
    # created. Mario loaded fine in the same session, which is what finally localised it.
    #
    # So the graft set is the union of the table's bone and every bone the mesh's influences name,
    # walked in SOURCE SKELETON ORDER so a parent is always grafted before its children. Each one
    # reparents to its source parent's merged index when that parent is present, and to the
    # table's anchor when it is not -- which for Mythra puts Sword1 under HaveR and Sword_lightC
    # and Sword_lightD under the grafted Sword1, exactly their source hierarchy.
    # ONE jq PASS, DELIBERATELY. The first version of this did the same work as a bash `for` loop
    # around three jq calls -- extract the bone names, look up each parent's name, look up that
    # name's index, append -- and it silently appended NOTHING for the skinned weapon while
    # reporting the right bone names. Same expression, same file, correct in isolation, wrong in
    # the loop. Rather than keep bisecting quoting layers, the whole graft is one expression with
    # no shell in the middle of it: `reduce` walks the graft set in source-skeleton order, so a
    # parent is always already appended when its children are resolved.
    jq --argjson anchor "$pidx" --arg m "$mesh" --arg b "$bone" \
       --slurpfile sm src_numshb.json --slurpfile sk src_nusktb.json '
      . as $doc
      | ( ( [ $sm[0].objects[] | select(.name == $m) ][0].bone_influences // [] )
        | map(.bone_name) ) + [$b]
      | unique as $want
      | [ $sk[0].bones[] | select(.name | IN($want[])) ] as $graft
      | reduce $graft[] as $g ($doc;
          if ([.bones[].name] | index($g.name)) != null then .
          else
            ( if ($g.parent_index == null or $g.parent_index < 0) then ""
              else ($sk[0].bones[$g.parent_index].name // "") end ) as $pname
            | ( [.bones[].name] | index($pname) ) as $found
            | .bones += [ $g | .parent_index = (if $found == null then $anchor else $found end) ]
          end
        )
    ' cur_nusktb.json > next_nusktb.json
    # SUBINDEX 0 ONLY. Several of these meshes appear more than once under one name -- Mythra's
    # `lightsword_VIS_O_OBJShape` has four and Palutena's staff two, which are the game's own LOD
    # and variant slots. Taking the first is what `set_mesh_visibility` can address by name and is
    # the highest-detail one.
    jq --arg n "$name" --arg m "$mesh" --slurpfile s src_numshb.json '
      .objects += [ ( [ $s[0].objects[] | select(.name == $m) ][0] | .name = $n | .subindex = 0 ) ]
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
