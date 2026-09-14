#!/usr/bin/env bash
# Generates the mod's config.json from what the mod tree ACTUALLY contains.
#
# WHY THIS IS GENERATED NOW. config.json's `new-dir-files` has to list every shipped path that
# vanilla Pikachu does not already have, or ARCropolis will not load it. It was hand-maintained,
# which makes it the same kind of parallel list that has already caused three separate defects
# here -- throw_f outside the dropin pre-flight, DONKEY missing from deforeign's SOURCES, every
# special missing from topscale's MAP. The failure mode is silent both ways: an undeclared file
# simply never loads, and a declared file that is absent injects a bogus entry into the search
# filesystem, which is a plausible cause of the load crashes this project has been chasing.
#
# So the rule is computed rather than remembered: a shipped file whose path EXISTS in vanilla
# Pikachu is a same-name replacement and needs no declaration; a shipped file whose path does not
# exist is new and must be declared. That is exactly the distinction, and the extraction has
# vanilla Pikachu on disk to compare against.
#
# ALIASES are the exception and stay hand-declared below, because they are not files in the mod
# tree at all -- `share-to-added` points a new Pikachu path at a file that already exists in the
# game's archive, so nothing is shipped and there is nothing to discover by walking.
#
# Regenerate with:  bash tools/gen-config.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MOD="$ROOT/pikachu-moveset/mod"
VAN="$ROOT/extracted/fighter/pikachu"
OUT="$MOD/config.json"

[ -d "$VAN" ] || { echo "no extracted vanilla pikachu at $VAN" >&2; exit 1; }

# target pikachu path | source path already in the game's archive
#
# THE GRAFTED WEAPONS' TEXTURES. `tools/merge-weapons.sh` merges each source fighter's weapon into
# Pikachu's body model as a named mesh, and each of those meshes carries its source material --
# which references textures by name. The names must resolve under Pikachu's own model directory,
# but the BYTES are already on the console, so every one is ALIASED rather than copied. That is
# what keeps this mod free of redistributed Nintendo texture data.
#
# Three per material, `_col` `_nor` `_prm`, read out of each source `model.numatb`. The fourth
# reference every one of them carries is `#replace_cubemap`, which is a built-in and needs no file.
#
# Keep this in step with merge-weapons.sh's TABLE; tools/check-weapons.sh fails if they disagree.
ALIASES="
fighter/pikachu/model/body/c00/met_chrom_001_col.nutexb|fighter/chrom/model/sword/c00/met_chrom_001_col.nutexb
fighter/pikachu/model/body/c00/met_chrom_001_nor.nutexb|fighter/chrom/model/sword/c00/met_chrom_001_nor.nutexb
fighter/pikachu/model/body/c00/met_chrom_001_prm.nutexb|fighter/chrom/model/sword/c00/met_chrom_001_prm.nutexb
fighter/pikachu/model/body/c00/def_cloud_004_col.nutexb|fighter/cloud/model/fusionsword/c00/def_cloud_004_col.nutexb
fighter/pikachu/model/body/c00/def_cloud_004_nor.nutexb|fighter/cloud/model/fusionsword/c00/def_cloud_004_nor.nutexb
fighter/pikachu/model/body/c00/def_cloud_004_prm.nutexb|fighter/cloud/model/fusionsword/c00/def_cloud_004_prm.nutexb
fighter/pikachu/model/body/c00/alp_light_002_col.nutexb|fighter/elight/model/esword/c00/alp_light_002_col.nutexb
fighter/pikachu/model/body/c00/alp_light_002_nor.nutexb|fighter/elight/model/esword/c00/alp_light_002_nor.nutexb
fighter/pikachu/model/body/c00/alp_light_002_prm.nutexb|fighter/elight/model/esword/c00/alp_light_002_prm.nutexb
fighter/pikachu/model/body/c00/metal_palutena_002_col.nutexb|fighter/palutena/model/body/c00/metal_palutena_002_col.nutexb
fighter/pikachu/model/body/c00/metal_palutena_002_nor.nutexb|fighter/palutena/model/body/c00/metal_palutena_002_nor.nutexb
fighter/pikachu/model/body/c00/metal_palutena_002_prm.nutexb|fighter/palutena/model/body/c00/metal_palutena_002_prm.nutexb
"

cd "$MOD"
# Files shipped under fighter/pikachu that vanilla does not have at the same path.
NEW=()
while IFS= read -r f; do
    rel="${f#./}"
    case "$rel" in fighter/pikachu/*) ;; *) continue ;; esac
    sub="${rel#fighter/pikachu/}"
    [ -e "$VAN/$sub" ] || NEW+=("$rel")
done < <(find . -type f | sort)

{
    echo '{'
    echo '    "new-dir-files": {'
    echo '        "fighter/pikachu/c00": ['
    first=1
    while IFS='|' read -r target _src; do
        [ -z "${target// }" ] && continue
        case "$target" in "#"*) continue ;; esac
        [ "$first" = 1 ] && first=0 || echo ','
        printf '            "%s"' "$target"
    done <<< "$ALIASES"
    for p in "${NEW[@]}"; do
        [ "$first" = 1 ] && first=0 || echo ','
        printf '            "%s"' "$p"
    done
    echo
    echo '        ]'
    echo '    },'
    echo '    "share-to-added": {'
    # GROUPED BY SOURCE, because `share-to-added` is a JSON OBJECT and two aliases sharing one
    # source file would otherwise emit the same key twice. Duplicate keys are not an error in most
    # parsers -- they silently keep the last -- so the second alias would vanish with no warning.
    # One vanilla file legitimately feeds several added paths, so this is reachable, not theoretical.
    first=1
    srcs=$(printf '%s\n' "$ALIASES" | grep -v '^#' | grep '|' | awk -F'|' '{print $2}' | sed 's/^ *//;s/ *$//' | awk '!seen[$0]++')
    while IFS= read -r s; do
        [ -z "$s" ] && continue
        [ "$first" = 1 ] && first=0 || echo ','
        printf '        "%s": [' "$s"
        tfirst=1
        while IFS='|' read -r target src; do
            [ -z "${target// }" ] && continue
            case "$target" in "#"*) continue ;; esac
            [ "$src" = "$s" ] || continue
            [ "$tfirst" = 1 ] && tfirst=0 || printf ', '
            printf '"%s"' "$target"
        done <<< "$ALIASES"
        printf ']'
    done <<< "$srcs"
    echo
    echo '    }'
    echo '}'
} > "$OUT"

n_alias=$(printf '%s\n' "$ALIASES" | grep -v '^#' | grep -c '|' || true)
echo "  wrote config.json: ${#NEW[@]} shipped new path(s) + $n_alias alias(es)"
