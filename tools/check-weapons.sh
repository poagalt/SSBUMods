#!/usr/bin/env bash
# The grafted weapons are described in THREE places. They must agree.
#
# WHY THIS EXISTS. `tools/merge-weapons.sh` decides what is in the model, `src/sword/mod.rs`
# decides when each mesh is shown, and `tools/gen-config.sh` decides which textures resolve.
# Nothing otherwise notices when one of the three moves and the others do not.
#
# AND THE MERGE IS NOT A BUILD STEP, WHICH IS WHY DRIFT IS POSSIBLE AT ALL. The merged model is
# gitignored along with everything else Nintendo-derived, so it is a LOCAL artefact that
# `merge-weapons.sh` has to be run by hand to produce. It cannot join build.sh either: three of the
# four weapons are merged from files pulled off the running console by `tools/arc-fetch.sh` into
# `work/arc-weapons`, which no clone of this repo has. So the model can be absent, or stale, and
# this check is the thing that says so -- the absence case prints the command to fix it.
#
# This project has paid for exactly that shape before: `tools/check-lists.sh` exists because
# gen-moves, moves.tsv and topscale's MAP were three hand-kept lists of the same 52 moves. A weapon
# that is in the table but not the model is an invisible mesh; one in the model but not the Rust is
# a blade Pikachu carries permanently; one whose textures are unaliased renders untextured or not
# at all.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
MODEL=pikachu-moveset/mod/fighter/pikachu/model/body/c00
W=$(mktemp -d); trap 'rm -rf "$W"' EXIT
rc=0

# The three lists, each read from its own file rather than restated here.
#
# THE RUST SIDE IS PARSED BY STRUCTURE, NOT BY LINE, and the first version of this check got that
# wrong -- it matched `^\s*\("name",` and so missed every entry written as a multi-line tuple,
# reporting two real weapons as "in the model but never shown". A checker whose failure mode is a
# false alarm trains people to ignore it.
#
# So: take the WEAPONS block, delete the motion arrays entirely (`&[ .. ]`), and every quoted
# string that remains is a mesh name by construction. The motion check below does the inverse and
# keeps only what is inside those arrays.
# `tail -n +2` DROPS THE DECLARATION LINE, and leaving it in broke this twice over. The type is
# `&[(&str, &[&str])]`, so the line itself contains `&[` -- and the array-stripping below then ate
# from the type's bracket through the first entry, silently losing chrom_sword. Only the entries
# are parsed here; the declaration carries no data.
WEAPONS_BLOCK=$(sed -n '/^const WEAPONS/,/^\];/p' pikachu-moveset/plugin/src/sword/mod.rs \
                | tail -n +2 | grep -v '^\s*//')
TABLE=$(awk -F'|' '/^[a-z_]+\|/{print $1}' tools/merge-weapons.sh | sort -u)
RUST=$(printf '%s' "$WEAPONS_BLOCK" | tr '\n' ' ' | sed 's/&\[[^]]*\]//g' \
       | grep -oE '"[a-z_]+"' | tr -d '"' | sort -u)

echo "== the merge table and the Rust visibility table name the same weapons =="
if [ "$TABLE" = "$RUST" ]; then
    echo "  ok  $(echo "$TABLE" | wc -l) weapon(s): $(echo $TABLE | tr '\n' ' ')"
else
    echo "  FAIL merge-weapons.sh and src/sword/mod.rs disagree:"
    comm -23 <(echo "$TABLE") <(echo "$RUST") | sed 's/^/       in the model but never shown: /'
    comm -13 <(echo "$TABLE") <(echo "$RUST") | sed 's/^/       shown but not in the model:   /'
    rc=1
fi

echo; echo "== every weapon is actually in the shipped model =="
if [ ! -f "$MODEL/model.numshb" ]; then
    echo "  FAIL no model at $MODEL -- run: bash tools/merge-weapons.sh"; rc=1
else
    "$T" "$MODEL/model.numshb"  "$W/mesh.json" >/dev/null 2>&1
    "$T" "$MODEL/model.nusktb"  "$W/bone.json" >/dev/null 2>&1
    "$T" "$MODEL/model.numdlb"  "$W/modl.json" >/dev/null 2>&1
    "$T" "$MODEL/model.numatb"  "$W/matl.json" >/dev/null 2>&1
    "$T" "$MODEL/model.numshexb" "$W/mx.json"  >/dev/null 2>&1
    while IFS='|' read -r name dir mesh material bone parent sphere radius; do
        case "${name:-}" in '' | \#*) continue ;; esac
        miss=""
        jq -e --arg n "$name" '[.objects[].name]|index($n)'            "$W/mesh.json" >/dev/null || miss="$miss mesh"
        jq -e --arg b "$bone" '[.bones[].name]|index($b)'              "$W/bone.json" >/dev/null || miss="$miss bone:$bone"
        jq -e --arg n "$name" '[.entries[].mesh_object_name]|index($n)' "$W/modl.json" >/dev/null || miss="$miss modl"
        jq -e --arg m "$material" '[.entries[].material_label]|index($m)' "$W/matl.json" >/dev/null || miss="$miss matl:$material"
        jq -e --arg n "$name" '[.mesh_object_groups[].mesh_object_name]|index($n)' "$W/mx.json" >/dev/null || miss="$miss meshex"
        if [ -n "$miss" ]; then
            echo "  FAIL $name is missing:$miss -- rerun tools/merge-weapons.sh"; rc=1
        else
            echo "  ok  $name (bone $bone, material $material)"
        fi
    done < <(awk -F'|' '/^[a-z_]+\|/' tools/merge-weapons.sh)
fi

echo; echo "== no skinned mesh names a bone the skeleton does not have =="
# THE CHECK THAT WOULD HAVE SAVED AN AFTERNOON OF PLAYTESTS.
#
# A graft carries the table's bone. Three of these weapons are RIGID -- `bone_influences` empty,
# mesh single-bound to one bone -- so one bone is exact, and Chrom's sword worked for weeks.
# Mythra's `lightsword_VIS_O_OBJShape` is SKINNED, weighted to Sword1, Sword_lightC and
# Sword_lightD, and the merge grafted only Sword1. The shipped model had two influences naming
# bones that do not exist.
#
# Vanilla Pikachu has ZERO such references, so this is a clean invariant and not a tolerance.
# What it cost: the training-mode load hung FOREVER on loading him. Not a crash -- no crash report,
# ARCropolis finished its filesystem work and went quiet, the plugin installed in full, the loading
# animation kept spinning, and no fighter object was ever created. Three boots, twenty-five minutes
# on the first. It was localised only when a vanilla Mario loaded fine in the same session.
#
# Every bone name, not just the grafted meshes': if a merge ever disturbs Pikachu's own skeleton
# this catches that too.
if [ -f "$MODEL/model.numshb" ]; then
    jq -r '.bones[].name' "$W/bone.json" > "$W/bonenames.txt"
    absent=$(jq -r '.objects[] | select(.bone_influences | length > 0)
                    | "\(.name)\t\(.bone_influences | map(.bone_name) | join(","))"' "$W/mesh.json" \
      | while IFS=$'\t' read -r mo bones; do
            for b in $(echo "$bones" | tr ',' ' '); do
                grep -qxF "$b" "$W/bonenames.txt" || echo "$mo needs $b"
            done
        done)
    if [ -n "$absent" ]; then
        echo "  FAIL a skinned mesh is weighted to a bone that is not in the skeleton:"
        printf '       %s\n' "$absent"
        echo "       rerun tools/merge-weapons.sh -- it grafts every bone a mesh's influences name"
        rc=1
    else
        echo "  ok  every influence bone of every skinned mesh is in the skeleton"
    fi
fi

echo; echo "== every mesh binds to the bone its move's animation drives =="
# THE ANIMATION DRIVES BONES BY NAME, and renaming a graft breaks it silently.
#
# Prefixing every graft with its weapon name (to dodge an add-bones collision) left
# `palutena_staff_Stick` and `chrom_sword_Sword` with no animation track at all, so the staff did
# not sweep and the Falchion sat in the wrong hand jabbing left. The hitboxes diverged from the
# mesh at the same time, because `tools/reach/src/bin/rebase.rs` looks the bone up as `Stick` and
# found add-bones' copy while the mesh was bound to the prefixed one.
#
# The collision it was dodging is not real: an animation supplies the transform per move, so one
# `Sword1` serves chrom's jab (animated) and mythra's dash (no track, wants a bind pose).
#
# So: the mesh's bind bone must be exactly the table's bone, and that bone must be in the skeleton.
if [ -f "$MODEL/model.numshb" ]; then
    bad=""
    while IFS='|' read -r name dir mesh material bone parent sphere radius; do
        case "${name:-}" in '' | \#*) continue ;; esac
        pb=$(jq -r --arg n "$name" '.objects[]|select(.name==$n)|.parent_bone_name' "$W/mesh.json" 2>/dev/null)
        [ "$pb" = "$bone" ] || bad="$bad ${name}:bound-to-${pb:-none}-not-${bone}"
        jq -e --arg b "$bone" '[.bones[].name]|index($b)' "$W/bone.json" >/dev/null 2>&1 \
            || bad="$bad ${name}:bone-${bone}-absent"
    done < <(awk -F'|' '/^[a-z_]+\|/' tools/merge-weapons.sh)
    if [ -n "$bad" ]; then
        echo "  FAIL mesh/bone binding is wrong:"; printf '       %s\n' $bad; rc=1
    else
        echo "  ok  every mesh binds to its table bone, and every one is in the skeleton"
    fi
fi
echo; echo "== no motion shows two weapons at once =="
# Only what is INSIDE the motion arrays, which is the inverse of the extraction above.
dupes=$(printf '%s' "$WEAPONS_BLOCK" | tr '\n' ' ' | grep -oE '&\[[^]]*\]' \
        | grep -oE '"[a-z_0-9]+"' | tr -d '"' | sort | uniq -d)
if [ -n "$dupes" ]; then
    echo "  FAIL these motions appear under more than one weapon:"; printf '       %s\n' $dupes; rc=1
else
    echo "  ok  every motion shows at most one weapon"
fi

echo; echo "== every material's textures actually resolve =="
# SHIPPED OR ALIASED, and the distinction cost most of a day.
#
# This used to assert "aliased", because the mod declared every weapon texture through
# `share-to-added` rather than shipping it -- so no Nintendo texture data sat in the tree.
#
# `share-to-added` CANNOT READ THE VANILLA ARC. It only resolves when a mod on the SD card
# provides a real file at the source path, and when it cannot, ARCropolis HANGS THE FIGHT LOAD
# rather than erroring. Chrom's three worked for weeks purely because the test console happens to
# carry an "Alucard over Chrom" mod that replaces those exact three textures; cloud's, mythra's and
# palutena's aliased into nothing. Episode 39's recipe SHIPS them, which is why it ships them.
#
# So the invariant is resolvability, not a mechanism: a referenced texture must be a real file in
# the mod tree or an alias whose source is one. `#replace_cubemap` is a built-in and needs neither.
MODELDIR=pikachu-moveset/mod/fighter/pikachu/model/body/c00
while IFS='|' read -r name dir mesh material bone parent sphere radius; do
    case "${name:-}" in '' | \#*) continue ;; esac
    src=""
    for root in extracted/fighter work/arc-weapons/fighter; do
        [ -f "$root/$dir/c00/model.numatb" ] && src="$root/$dir/c00" && break
    done
    if [ -z "$src" ]; then
        echo "  ..  $name: source numatb not on disk, cannot check its textures"
        continue
    fi
    "$T" "$src/model.numatb" "$W/src.json" >/dev/null 2>&1
    for tex in $(jq -r --arg m "$material" '.entries[]|select(.material_label==$m)|.textures[]?|.data' "$W/src.json" 2>/dev/null | grep -v '^#'); do
        if [ -f "$MODELDIR/${tex}.nutexb" ]; then
            :
        elif grep -q "/${tex}\.nutexb|" tools/gen-config.sh; then
            echo "  !!  $name: ${tex}.nutexb is ALIASED, not shipped -- share-to-added only resolves"
            echo "      if another mod happens to provide that source path. Ship it instead."
        else
            echo "  FAIL $name material $material references $tex.nutexb -- not shipped and not aliased"; rc=1
        fi
    done
done < <(awk -F'|' '/^[a-z_]+\|/' tools/merge-weapons.sh)
[ $rc -eq 0 ] && echo "  ok  every referenced texture is a real file in the shipped model directory"

echo
if [ $rc -ne 0 ]; then echo "WEAPON CHECK FAILED"; exit 1; fi
echo "weapons: OK"
