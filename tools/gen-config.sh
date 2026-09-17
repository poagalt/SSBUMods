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
ALIASES=""

# THE ORACLE IS THE ARCHIVE MAP, NOT THE LOCAL EXTRACTION, and using the extraction mis-declared
# 308 paths the moment the mod started shipping more than one costume.
#
# `extracted/fighter/pikachu` is a PARTIAL mirror: it has all 338 motion files for c00 and FIVE
# for c01, because models and per-costume animations were never pulled (see
# docs/the-data-analysis.md -- the extraction is complete for params, motion lists and skeletons
# and deliberately skipped the rest). Testing "does vanilla have this path" against it therefore
# answers NO for 44 of the 75 animations in each of c01..c07 -- files vanilla certainly does have
# -- and declared 308 same-name replacements as new additions.
#
# `work/acquire/arc-paths.txt` is the real thing: all 590,711 paths in the archive. Asking it makes
# the distinction this file's header describes actually correct rather than approximately correct.
# The extraction stays as a fallback so a clone without the map still builds, and which oracle was
# used is printed, because the two do not agree and a silent difference of 308 entries is exactly
# the kind of thing that has hung a fight load here before.
ARCMAP="$ROOT/work/acquire/arc-paths.txt"
VANLIST=$(mktemp); SHIPPED=$(mktemp); trap 'rm -f "$VANLIST" "$SHIPPED"' EXIT
if [ -s "$ARCMAP" ]; then
    ORACLE="archive map ($(grep -c '^fighter/pikachu/' "$ARCMAP") vanilla pikachu path(s))"
    grep '^fighter/pikachu/' "$ARCMAP" | sort -u > "$VANLIST"
else
    ORACLE="local extraction -- PARTIAL, may over-declare; pull work/acquire/arc-paths.txt"
    ( cd "$VAN" && find . -type f | sed 's|^\./|fighter/pikachu/|' ) | sort -u > "$VANLIST"
fi

cd "$MOD"
find . -type f | sed 's|^\./||' | grep '^fighter/pikachu/' | sort -u > "$SHIPPED"
# Files shipped under fighter/pikachu that vanilla does not have at the same path.
NEW=()
while IFS= read -r rel; do
    [ -n "$rel" ] && NEW+=("$rel")
done < <(comm -23 "$SHIPPED" "$VANLIST")

{
    echo '{'
    echo '    "new-dir-files": {'
    # ONE KEY PER COSTUME, BECAUSE THE KEY *IS* THE DIRECTORY THE GAME LOADS.
    #
    # `new-dir-files` maps an arc DIRECTORY to the extra file paths to add to it. A Pikachu in
    # costume 5 loads `fighter/pikachu/c05`, so a `.../motion/body/c05/x.nuanmb` declared under
    # the `fighter/pikachu/c00` key is added to the directory a c00 Pikachu loads and to nowhere
    # else -- it would simply never resolve for the costume it belongs to.
    #
    # Everything was under the single c00 key while the mod shipped only c00, and that was
    # correct. The moment `mirror-costumes.sh` began shipping c01..c07 it silently became 210
    # declarations attached to the wrong directory: the files would be on the card, declared,
    # verified, and still invisible to seven of the eight costumes.
    #
    # The costume comes from the path's own `c0N` component rather than from a list, so a future
    # `c08` or a `model/<article>/c03` needs no change here. A shipped path with no costume
    # component at all is an error worth failing on: there is no directory to attach it to.
    keyfirst=1
    for c in $(printf '%s\n' "${NEW[@]}" | grep -oE '/c[0-9]{2}/' | tr -d '/' | sort -u); do
        [ "$keyfirst" = 1 ] && keyfirst=0 || echo ','
        printf '        "fighter/pikachu/%s": [\n' "$c"
        first=1
        # Aliases are c00-only by construction: they name a target path under c00.
        if [ "$c" = "c00" ]; then
            while IFS='|' read -r target _src; do
                [ -z "${target// }" ] && continue
                case "$target" in "#"*) continue ;; esac
                [ "$first" = 1 ] && first=0 || echo ','
                printf '            "%s"' "$target"
            done <<< "$ALIASES"
        fi
        for p in "${NEW[@]}"; do
            case "$p" in *"/$c/"*) ;; *) continue ;; esac
            [ "$first" = 1 ] && first=0 || echo ','
            printf '            "%s"' "$p"
        done
        echo
        printf '        ]'
    done
    echo
    echo '    },'
    echo '    "share-to-added": {'
    # GROUPED BY SOURCE, because `share-to-added` is a JSON OBJECT and two aliases sharing one
    # source file would otherwise emit the same key twice. Duplicate keys are not an error in most
    # parsers -- they silently keep the last -- so the second alias would vanish with no warning.
    # One vanilla file legitimately feeds several added paths, so this is reachable, not theoretical.
    first=1
    srcs=$(printf '%s\n' "$ALIASES" | grep -v '^#' | grep '|' | awk -F'|' '{print $2}' | sed 's/^ *//;s/ *$//' | awk '!seen[$0]++' || true)
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
echo "    oracle: $ORACLE"
