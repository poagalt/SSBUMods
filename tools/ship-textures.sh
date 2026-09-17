#!/usr/bin/env bash
# Ships every texture the grafted weapons' materials actually reference, derived from the merged
# model rather than from a list.
#
# WHY THIS EXISTS. The twelve textures the first four weapons needed were copied into the mod tree
# BY HAND, and nothing checked that the set was complete. Then `tools/merge-weapons.sh` started
# grafting every subindex of a mesh instead of only the first -- the fix for Mythra's 13%-of-a-blade
# and for Corrin's missing sword -- and each new subindex brought its OWN material with its own
# textures. Sixteen files appeared out of nowhere: four for `def_light_004`, four for
# `def_light_005`, seven for Corrin's three `def_sword_*`, and `default_normal`.
#
# A hand-copied set cannot survive that, and the failure is silent in the worst direction: the
# material resolves to nothing and the mesh renders untextured or not at all, which looks exactly
# like a graft that did not work. `tools/check-weapons.sh` catches it now, but catching it is not
# the same as never creating it.
#
# So the rule is computed: for every weapon in merge-weapons.sh's TABLE, read the materials the
# MERGED model maps that mesh to, read the textures those materials reference out of the merged
# `model.numatb`, and copy each from the weapon's own source fighter directory. Adding a weapon,
# or a subindex, cannot omit a texture any more.
#
# A `#`-prefixed reference is a built-in -- `#replace_cubemap` is the common one -- and needs no
# file. `default_Normal` is NOT one of those: it is a real `default_normal.nutexb` in the source
# fighter's directory, and the archive is lowercase-normalised, which is why the lookup here is
# case-insensitive and the file ships under its archive name.
#
# ====================================================================================
# `jq` HERE EMITS CRLF, AND IT COST AN HOUR. Read this before touching a jq pipeline.
# ====================================================================================
#
# This is a Windows jq writing stdout in text mode, so every line it prints ends CR LF -- the CR
# is NOT in the data. Command substitution then strips the trailing newline but NOT the carriage
# return, so word-splitting yields `met_chrom_001_col<CR>` and the existence test looks for
# `met_chrom_001_col<CR>.nutexb` and never finds it.
#
# WHAT MADE IT LOOK LIKE SOMETHING ELSE: the LAST token of each list comes out clean, because its
# CR LF is the one at the very end that substitution trims. So exactly one texture per material
# worked -- the alphabetically last, `_prm` -- and the script reported `_col` and `_nor` missing
# while sitting next to them on disk. That reads like a path bug, and two path "fixes" were made
# before the bytes were looked at with `od -c`.
#
# Every jq pipeline whose output is split into words is piped through `tr -d` of CR for that
# reason. Single-value captures need it too, for the same reason.
#
# THE MOD TREE IS GITIGNORED. These are Nintendo's texture bytes; they are assembled locally and
# on the card and never committed, which is the same position the twelve originals were shipped
# under once `share-to-added` turned out to be unable to read the vanilla arc.
#
# Regenerate with:  bash tools/ship-textures.sh
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
MODEL="$ROOT/pikachu-moveset/mod/fighter/pikachu/model/body/c00"
W="$ROOT/work/shiptex"
CR=$(printf '\r')

[ -f "$MODEL/model.numdlb" ] || { echo "no merged model at $MODEL -- run tools/merge-weapons.sh first" >&2; exit 1; }
rm -rf "$W"; mkdir -p "$W"
"$T" "$MODEL/model.numdlb" "$W/modl.json" >/dev/null 2>&1
"$T" "$MODEL/model.numatb" "$W/matl.json" >/dev/null 2>&1

shipped=0; skipped=0; missing=""
while IFS='|' read -r name dir mesh material bone parent sphere radius; do
    case "${name:-}" in '' | \#*) continue ;; esac

    # BOTH ROOTS ARE SEARCHED FOR EVERY FILE, not one root chosen per weapon. The model
    # definitions and the textures are not in the same place: Chrom's sword model is in
    # `extracted/` while every texture past it came from `arc-fetch.sh` into `work/arc-weapons/`,
    # and picking a root by `-d` found the two-file extracted directory and then reported eleven
    # textures missing that were sitting in the other one.
    srcs=""
    for root in "$ROOT/extracted/fighter" "$ROOT/work/arc-weapons/fighter"; do
        [ -d "$root/$dir/c00" ] && srcs="$srcs $root/$dir/c00"
    done
    [ -n "${srcs// }" ] || { missing="$missing
  $name: no source directory for $dir"; continue; }

    # Materials come from the MERGED model, keyed on the graft's own mesh name -- so every
    # subindex is covered, not just the one the TABLE happens to name.
    mats=$(jq -r --arg n "$name" '[.entries[]|select(.mesh_object_name==$n)|.material_label]|unique|.[]' \
           "$W/modl.json" 2>/dev/null | tr -d "$CR")
    [ -n "$mats" ] || { missing="$missing
  $name: no material entries in the merged numdlb"; continue; }

    for lbl in $mats; do
        texs=$(jq -r --arg l "$lbl" '.entries[]|select(.material_label==$l)|.textures[]?.data' \
               "$W/matl.json" 2>/dev/null | tr -d "$CR" | sort -u)
        for t in $texs; do
            case "$t" in \#*) continue ;; esac          # built-in, no file
            if [ -f "$MODEL/$t.nutexb" ]; then skipped=$((skipped+1)); continue; fi
            # Case-insensitive, because a material may say `default_Normal` for a file the archive
            # stores as `default_normal.nutexb`.
            found=$(find $srcs -maxdepth 1 -iname "$t.nutexb" 2>/dev/null | head -1)
            if [ -n "$found" ]; then
                cp "$found" "$MODEL/$(basename "$found")" && shipped=$((shipped+1))
            else
                missing="$missing
  $name material $lbl needs $t.nutexb -- not in${srcs//$ROOT/}"
            fi
        done
    done
done < <(awk -F'|' '/^[a-z_]+\|/' tools/merge-weapons.sh)

echo "  textures: $shipped copied, $skipped already present"
if [ -n "$missing" ]; then
    echo "TEXTURES MISSING FROM THE SOURCE TREE:$missing" >&2
    echo "Pull them with tools/arc-fetch.sh (the game has to be running)." >&2
    exit 1
fi
