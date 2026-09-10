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
# Chrom's sword metal textures: the jab's hitboxes ride Sword1, the bone was grafted onto Pikachu's
# skeleton, and the material references these three. Aliased rather than copied because the bytes
# are already on the console.
ALIASES="
fighter/pikachu/model/body/c00/met_chrom_001_col.nutexb|fighter/chrom/model/sword/c00/met_chrom_001_col.nutexb
fighter/pikachu/model/body/c00/met_chrom_001_nor.nutexb|fighter/chrom/model/sword/c00/met_chrom_001_nor.nutexb
fighter/pikachu/model/body/c00/met_chrom_001_prm.nutexb|fighter/chrom/model/sword/c00/met_chrom_001_prm.nutexb
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
    first=1
    while IFS='|' read -r target src; do
        [ -z "${target// }" ] && continue
        [ "$first" = 1 ] && first=0 || echo ','
        printf '        "%s": ["%s"]' "$src" "$target"
    done <<< "$ALIASES"
    echo
    echo '    }'
    echo '}'
} > "$OUT"

n_alias=$(printf '%s\n' "$ALIASES" | grep -c '|' || true)
echo "  wrote config.json: ${#NEW[@]} shipped new path(s) + $n_alias alias(es)"
