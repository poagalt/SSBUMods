#!/usr/bin/env bash
# Ships a cloned article's model and motion files under the HOST fighter's paths, or removes them
# when the clone is off.
#
# THIS IS WHY THE CLONE CRASHED. A cloned weapon is owned by pikachu, so the game looks for
# `fighter/pikachu/model/spikeball/c00/...` and `fighter/pikachu/motion/spikeball/c00/...`. The mod
# shipped NONE of it -- config.json never mentioned spikeball at all -- and loading into a match
# with Pikachu died in the game's own weapon-creation path with a null dereference at +0x138, no
# smashline frame in the crashed thread.
#
# vendor/ssbu-mod-workshop/episode46/ArticleCloningGuide.md step 4 says exactly this: copy the
# article's assets across under the host's paths. The note in src/articles/mod.rs claimed those
# files "are shipped" -- the third wrong claim in that file, and the one that cost a crash.
#
# ONLY c00 IS SHIPPED, AND THAT IS A KNOWN HOLE. A cloned article resolves under the OWNER's
# costume slot -- `fighter/pikachu/model/spikeball/c0N` for whichever Pikachu is selected -- and
# vanilla has no `fighter/pikachu/spikeball` at any slot, so there is nothing to fall back to. On
# a non-c00 Pikachu the model path resolves to nothing. Deliberately left until the clone is
# proven on c00, because shipping eight costumes is 192 extra declared paths per article and this
# is one variable at a time; the fix is a `for c in c00..c07` around the copy below.
#
# GATED ON CLONE_ENABLED, and removal matters as much as copying: a declared-but-absent path
# injects a bogus entry into the search filesystem, and shipped-but-undeclared files never load.
# tools/gen-config.sh derives the declarations from whatever this leaves in the tree, so the two
# cannot disagree.
#
# Regenerate with:  bash tools/gen-article-assets.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MOD="$ROOT/pikachu-moveset/mod/fighter/pikachu"
SRC="$ROOT/extracted/fighter"
A="$ROOT/pikachu-moveset/plugin/src/articles/mod.rs"

# host article name | donor fighter | donor article name
ARTICLES="
spikeball|packun|spikeball
"

clone_on=$(grep -cx 'pub const CLONE_ENABLED: bool = true;' "$A" || true)

while IFS='|' read -r name donor donor_name; do
    [ -z "${name// }" ] && continue
    for kind in model motion; do
        dst="$MOD/$kind/$name/c00"
        src="$SRC/$donor/$kind/$donor_name/c00"
        if [ "$clone_on" = "0" ]; then
            if [ -d "$dst" ]; then
                rm -rf "$MOD/$kind/$name"
                echo "  removed $kind/$name (clone disabled)"
            fi
            continue
        fi
        [ -d "$src" ] || { echo "  !! $src not extracted -- cannot ship $kind/$name" >&2; exit 1; }
        mkdir -p "$dst"
        n=0
        for f in "$src"/*; do
            [ -f "$f" ] || continue
            cp -f "$f" "$dst/$(basename "$f")"
            n=$((n+1))
        done
        echo "  shipped $kind/$name/c00: $n file(s) from $donor"
    done
done <<< "$ARTICLES"

[ "$clone_on" = "0" ] && echo "  (clone disabled -- no article assets shipped)"
exit 0
