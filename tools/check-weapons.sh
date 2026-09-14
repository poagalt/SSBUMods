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

echo; echo "== no motion shows two weapons at once =="
# Only what is INSIDE the motion arrays, which is the inverse of the extraction above.
dupes=$(printf '%s' "$WEAPONS_BLOCK" | tr '\n' ' ' | grep -oE '&\[[^]]*\]' \
        | grep -oE '"[a-z_0-9]+"' | tr -d '"' | sort | uniq -d)
if [ -n "$dupes" ]; then
    echo "  FAIL these motions appear under more than one weapon:"; printf '       %s\n' $dupes; rc=1
else
    echo "  ok  every motion shows at most one weapon"
fi

echo; echo "== every material's textures are aliased in gen-config =="
while IFS='|' read -r name dir mesh material bone parent sphere radius; do
    case "${name:-}" in '' | \#*) continue ;; esac
    # `#replace_cubemap` is a built-in and needs no file; everything else must be aliased.
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
        if grep -q "/${tex}\.nutexb|" tools/gen-config.sh; then
            :
        else
            echo "  FAIL $name material $material references $tex.nutexb with no alias in gen-config.sh"; rc=1
        fi
    done
done < <(awk -F'|' '/^[a-z_]+\|/' tools/merge-weapons.sh)
[ $rc -eq 0 ] && echo "  ok  every referenced texture is aliased"

echo
if [ $rc -ne 0 ]; then echo "WEAPON CHECK FAILED"; exit 1; fi
echo "weapons: OK"
