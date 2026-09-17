#!/usr/bin/env bash
# Merges every source fighter's weapon into Pikachu's body model as toggleable meshes.
#
# GENERALISES tools/merge-sword.sh, which did Chrom's Falchion and nothing else. The technique is
# unchanged -- LilyLavender Modding Workshop episode 39: give the mesh object a distinct name so
# `ModelModule::set_mesh_visibility` can target it, single-bind it to a bone grafted under
# Pikachu's own skeleton, and SHIP the textures, declared through new-dir-files.
#
# SHIP, not alias. `share-to-added` cannot read the vanilla arc -- it resolves only when a mod on
# the SD card provides a real file at the source path, and when it cannot, ARCropolis HANGS the
# fight load rather than erroring. Chrom's worked for weeks only because the test console carries
# an "Alucard over Chrom" mod replacing those exact three textures. The mod tree is gitignored, so
# shipping them redistributes nothing.
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
corrin_sword|kamui/model/body|Kamui_Weapon_VIS_O_OBJShape|def_sword_001|HaveR|HandR|-0.000500202,5.6193275,-0.0007674639|7.7049875
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
    # BARE NAMES, BECAUSE THE ANIMATION DRIVES BONES BY NAME.
    #
    # Prefixing every graft with the weapon name removed the add-bones collision and BROKE all
    # three weapons that need animating. The dropped-in `.nuanmb` animates bones by name:
    #
    #     c00attack11.nuanmb   (chrom jab)     HaveR Sword1 Sword2
    #     c05attackairn.nuanmb (palutena nair) HaveR Stick
    #     e00catch*.nuanmb     (palutena grab) HaveR Stick
    #     c00attackdash.nuanmb (mythra dash)   HaveR only
    #
    # A graft called `palutena_staff_Stick` has no track, so the staff sat at bind pose and did not
    # sweep; `chrom_sword_Sword` likewise, which is why the Falchion was in the wrong hand jabbing
    # left. Playtest, exactly: "Chrom sword doesn't appear in pikachu's correct hand when jabbing to
    # the left. Palutena staff doesn't spin correctly when using nair."
    #
    # THE COLLISION IS NOT A PROBLEM AFTER ALL, which is what made prefixing look necessary.
    # `add-bones.sh` wants `Sword1` for chrom's jab and this tool wants one for mythra's sword --
    # different bones, same name. But an ANIMATION supplies the transform per move, so one `Sword1`
    # serves both: chrom's jab animates it, and mythra's dash has no track and therefore wants a
    # bind pose, which is hers. The bind only decides where an UNANIMATED bone sits.
    #
    # Two skips, both load-bearing. Already in the skeleton: it is the same bone by name, leave it.
    # Not in the SOURCE skeleton: chrom's table bone is `Sword1` and his sword MODEL only carries
    # `HaveR` and `Sword` -- `Sword1` lives in his BODY skeleton, which add-bones.sh supplies from
    # work/fk/chrom_skel.json. The mesh binds to the name either way.
    # THE GRAFT SET IS THE TABLE'S BONE PLUS EVERY BONE THE MESH'S INFLUENCES NAME, walked in
    # SOURCE SKELETON ORDER so a parent is always appended before its children. Bare names, because
    # the dropped-in animation drives bones BY NAME -- see the note below the table.
    #
    # A SKINNED GRAFT IS FINE, and believing otherwise cost Mythra's sword. Her
    # `lightsword_VIS_O_OBJShape` is weighted 672 vertices to Sword1 and 32 each to Sword_lightC
    # and Sword_lightD. I forced every graft rigid on the theory that a skinned mesh needs a
    # runtime matrix palette entry for bones that are not in the transform chain -- reasoned from
    # rebase.rs's proof that grafted bones are not in that chain, and motivated by a load hang that
    # turned out to be `share-to-added` instead. Forcing it rigid reinterprets model-space vertices
    # as bone-local and collapses the blade: "Mythra sword doesn't appear, but if I go
    # frame-by-frame I can see very small slivers where the middle of the sword would be".
    #
    # So the weights are left alone and the bones they name are grafted. Two skips: already in the
    # skeleton (same bone by name, leave it), and absent from the SOURCE skeleton -- chrom's table
    # bone is `Sword1` and his sword MODEL carries only `HaveR` and `Sword`, because `Sword1` lives
    # in his BODY skeleton and `add-bones.sh` supplies it.
    # THE BONE SET IS UNIONED ACROSS EVERY SUBINDEX, not read off the first one. Mythra's four
    # subindices name four DIFFERENT bone sets -- sub 0 has Sword1/Sword_lightC/Sword_lightD, and
    # sub 2 (the 6,876-vertex bulk of the blade) needs nine: Sword1 plus Sword_lightA, B, C, D,
    # E1, E2, F1, F2. Taking sub 0's three left six of the nine ungrafted, so even the bones the
    # grafted geometry was weighted to were missing.
    jq --argjson anchor "$pidx" --arg m "$mesh" --arg b "$bone" \
       --slurpfile sm src_numshb.json --slurpfile sk src_nusktb.json '
      . as $doc
      | ( ( [ $sm[0].objects[] | select(.name == $m) | (.bone_influences // [])[] | .bone_name ] )
          + [$b] )
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

    # EVERY SUBINDEX IS GRAFTED, AND TAKING ONLY THE FIRST WAS THE BUG.
    #
    # This block used to say "SUBINDEX 0 ONLY ... the game's own LOD and variant slots. Taking the
    # first is what `set_mesh_visibility` can address by name and is the highest-detail one."
    # Both halves of that are false, and the playtest reports were the two weapons it broke.
    #
    # THEY ARE NOT LOD SLOTS. LOD is a separate mesh NAME with a `_low` suffix -- the same files
    # carry `lightsword_lowShape`, `staff_group_lowShape`, `body_lowShape`. Several subindices
    # under ONE name are MATERIAL SPLITS: one draw call per material, each with its own entry in
    # `model.numdlb`. Mythra's four are `alp_light_002`, `def_light_004`, `def_light_005`,
    # `def_light_006`.
    #
    # AND SUBINDEX 0 IS NOT THE HIGHEST DETAIL. Measured, per subindex, in vertex indices:
    #
    #     mythra  lightsword_VIS_O_OBJShape   1320 /  936 / 6876 /  912    sub 0 is 13%
    #     corrin  Kamui_Weapon_VIS_O_OBJShape 11964 /  168 / 2976         sub 0 is 79%
    #     palutena staff_group_VIS_O_OBJShape  2442 /  684                sub 0 is 78%
    #     chrom   fal_RShape                  single subindex
    #     cloud   fusionsword1_VIS_O_OBJShape  single subindex
    #
    # That table is the whole diagnosis. The two weapons that worked in game are exactly the two
    # with one subindex; the two that were reported broken are exactly the two with more, and
    # 13% of a blade is "Mythra sword doesn't appear, but if I go frame-by-frame I can see very
    # small slivers where the middle of the sword would be" -- the playtest was describing sub 0.
    #
    # THE MATERIALS COME FROM THE SOURCE `numdlb`, NOT FROM THE TABLE. The table's `material`
    # column can only name one, which is how three of Mythra's four went missing without a word.
    # It is kept as an ASSERTION against subindex 0 below, so a stale table fails loudly instead
    # of being quietly overridden.
    #
    # `parent_bone_name` is overridden ONLY for a rigid mesh, and it has to be: chrom's source
    # mesh single-binds to `Sword`, which no animation drives, while the table names `Sword1`,
    # which his jab animates. A SKINNED mesh keeps its own weights and an empty parent, which is
    # how the game authored it -- and forcing one rigid reinterprets model-space vertices as
    # bone-local and collapses the blade.
    nsub=$(jq --arg m "$mesh" '[.objects[] | select(.name == $m)] | length' src_numshb.json)
    sub0mat=$(jq -r --arg m "$mesh" '
        [.entries[] | select(.mesh_object_name == $m)] | sort_by(.mesh_object_subindex)
        | .[0].material_label // ""' src_numdlb.json)
    if [ "$sub0mat" != "$material" ]; then
        echo "TABLE STALE for $name: subindex 0 of $mesh uses material '$sub0mat', table says '$material'" >&2
        exit 1
    fi

    jq --arg n "$name" --arg m "$mesh" --arg pb "$bone" --slurpfile s src_numshb.json '
      ( [ $s[0].objects[] | select(.name == $m) ] | sort_by(.subindex) ) as $objs
      | .objects += [ $objs[]
          | .name = $n
          | .parent_bone_name = ( if ((.bone_influences // []) | length) == 0 then $pb
                                  else (.parent_bone_name // "") end ) ]
    ' cur_numshb.json > next_numshb.json

    # One modl entry per subindex, each keeping the source's own subindex AND its own material.
    jq --arg n "$name" --arg m "$mesh" --slurpfile d src_numdlb.json '
      ( [ $d[0].entries[] | select(.mesh_object_name == $m) ]
        | sort_by(.mesh_object_subindex) ) as $es
      | .entries += [ $es[] | .mesh_object_name = $n ]
    ' cur_numdlb.json > next_numdlb.json

    # Every material any subindex uses, appended only if Pikachu does not already carry that label
    # -- two weapons from one fighter would otherwise duplicate it and the game rejects duplicate
    # labels. `reduce` because there are now several per weapon and each has to see the result of
    # the last.
    jq --arg m "$mesh" --slurpfile d src_numdlb.json --slurpfile a src_numatb.json '
      ( [ $d[0].entries[] | select(.mesh_object_name == $m) | .material_label ] | unique ) as $mats
      | reduce $mats[] as $lbl (.;
          if ([.entries[] | .material_label] | index($lbl)) then .
          else .entries += [ ( $a[0].entries[] | select(.material_label == $lbl) ) ] end
        )
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
