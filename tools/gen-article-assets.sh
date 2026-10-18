#!/usr/bin/env bash
# Ships every enabled cloned article's model and motion under the HOST fighter's paths, and removes
# the ones that are not enabled.
#
# A cloned weapon is owned by pikachu, so the game looks for
# `fighter/pikachu/model/<name>/cNN/...` and `fighter/pikachu/motion/<name>/cNN/...`. The mod once
# shipped NONE of it -- config.json never mentioned the article at all -- which is step 4 of
# vendor/ssbu-mod-workshop/episode46/ArticleCloningGuide.md and was skipped.
#
# It was NOT the crash, and that mattered. The header here used to claim it was. The crash is
# `WeaponSpecializer_PackunSpikeball` reading Piranha Plant's `FighterParamAccessor2` entry with the
# fighter kind compiled in, root-caused in docs/the-article-path.md; the assets were present and
# declared for the last ten boots and it crashed every time. Shipping them is still necessary --
# just not sufficient, and not the fault.
#
# WHICH COSTUME SLOTS, AND WHY THE DEFAULT IS ONE OF THEM.
#
# A cloned article resolves under the OWNER's costume slot, and vanilla has no
# `fighter/pikachu/<name>` at any slot, so on a cNN Pikachu with no cNN article the path resolves
# to nothing. Shipping all eight is therefore what a RELEASE needs.
#
# It is not what iterating needs, and the difference is twelve minutes a deploy. sys-ftpd moves
# 0.96 files/sec -- measured over a 716-file run, and connection reuse is not available, because
# batching listings down one control connection was a 62-second REGRESSION recorded in
# tools/deploy.sh. So the cost is per FILE, and the eight slots are almost entirely the same file:
# chrom/sword is 14 of 14 model files byte-identical across all eight, elight/esword 23 of 23,
# cloud/fusionsword 13 of 14, gamewatch/parachute 13 of 16. Eight copies of the same bytes at a
# second each.
#
#   ARTICLE_SLOTS=c00          (default) one slot, ~90 files, ~1.5 min
#   ARTICLE_SLOTS=all          every slot the donor has, ~716 files, ~12.4 min
#   ARTICLE_SLOTS="c00 c03"    an explicit list
#
# SHRINKING COSTS ONE PRUNE. tools/deploy.sh retires remote files the local tree no longer ships,
# one DELE each, so the first deploy after going from `all` to `c00` deletes the other seven slots
# at the same 0.96/sec. That is paid once and then every deploy is the short one.
ARTICLE_SLOTS="${ARTICLE_SLOTS:-c00}"
#
# TWO SOURCE ROOTS, SEARCHED IN ORDER. `work/arc-vis` holds 1764 files pulled off the running game
# for exactly this -- complete model and motion sets for every candidate across all eight slots --
# and `extracted/` is the older partial copy that has, for instance, one file of
# `cloud/model/fusionsword`. arc-vis wins where both have a path. Neither is in git: both are
# Nintendo-derived, and `tools/arc-fetch.sh` rebuilds arc-vis from a running console.
#
# THE ARTICLES ARE READ FROM THE RUST. src/articles/mod.rs holds the candidate table with a
# per-row `enabled`; this ships every row that is enabled AND clean, and removes every other
# candidate's assets, so flipping a row off cannot leave its files behind as dead declared paths.
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
A="$ROOT/pikachu-moveset/plugin/src/articles/mod.rs"
SRC_ROOTS=("$ROOT/work/arc-vis/fighter" "$ROOT/extracted/fighter")

clone_on=$(grep -cx 'pub const CLONE_ENABLED: bool = true;' "$A" || true)

# host_name|donor|donor_article|clean|enabled|authored, one line per Candidate row.
rows=$(sed -n 's/.*Candidate *{ *host_name: *"\([^"]*\)" *, *donor: *"\([^"]*\)" *, *donor_article: *"\([^"]*\)" *, *weapon_kind: *"[^"]*" *, *clean: *\([a-z]*\) *, *enabled: *\([a-z]*\) *, *authored: *\([a-z]*\) *, *model_from: *"\([^"]*\)".*/\1|\2|\3|\4|\5|\6|\7/p' "$A")
[ -n "$rows" ] || { echo "  !! no Candidate rows parsed from src/articles/mod.rs" >&2; exit 1; }

# Resolve a donor article directory against the source roots, most complete first.
find_src() {
    local rel="$1" r
    for r in "${SRC_ROOTS[@]}"; do
        if [ -d "$r/$rel" ] && [ -n "$(ls -A "$r/$rel" 2>/dev/null)" ]; then printf '%s\n' "$r/$rel"; return 0; fi
    done
    return 1
}

shipped_any=0
while IFS='|' read -r name donor donor_name clean enabled authored model_from; do
    [ -z "${name// }" ] && continue
    want=0
    [ "$clone_on" != "0" ] && [ "$enabled" = "true" ] && [ "$clean" = "true" ] && want=1
    # A DIRTY ROW THAT src/articles/neutralize.rs COVERS SHIPS TOO: its unguarded donor reads are
    # repointed at the engine's own `ret` default for Pikachu's instances (gated by
    # tools/check-article-donor-params.sh), and the plugin never spawns it unless the hook's site
    # verified at runtime. Read from that file so the two cannot disagree.
    if [ "$clone_on" != "0" ] && [ "$enabled" = "true" ] && [ "$clean" = "false" ]        && grep -q "(\"$name\", &\[" "$ROOT/pikachu-moveset/plugin/src/articles/neutralize.rs"; then
        want=1
    fi

    if [ "$want" = "0" ]; then
        for kind in model motion; do
            if [ -d "$MOD/$kind/$name" ]; then
                rm -rf "$MOD/$kind/$name"
                echo "  removed $kind/$name"
            fi
        done
        continue
    fi

    # ALWAYS CLEARED FIRST, so shrinking ARTICLE_SLOTS actually shrinks the tree. Copying over the
    # top would leave c01..c07 behind from a previous `all` run, and gen-config.sh would go on
    # declaring them -- which is the "declared but absent" state this script exists to make
    # unreachable, arrived at from the other direction.
    for kind in model motion; do rm -rf "$MOD/$kind/$name"; done

    # AUTHORED ROWS ARE BUILT, NOT COPIED. Palutena's staff and Corrin's Yato are not articles in
    # the game at all -- they are meshes inside their owner's body model -- so there is nothing to
    # copy verbatim. tools/author-article.py assembles a two-bone article model from the donor's
    # body model and takes the motion directory from the host kind, and it owns both directories
    # for that row, so this hands over entirely rather than doing half the job first.
    if [ "$authored" = "true" ]; then
        PY=""
        for c in py python3 python; do command -v "$c" >/dev/null 2>&1 && { PY="$c"; break; }; done
        [ -n "$PY" ] || { echo "  !! no python on PATH -- cannot author '$name'" >&2; exit 1; }
        "$PY" "$ROOT/tools/author-article.py" "$name" || exit 1
        shipped_any=1
        continue
    fi

    total=0
    for kind in model motion; do
        # A LENT MODEL, DECLARED IN THE TABLE RATHER THAN SPECIAL-CASED HERE.
        #
        # `model_from` names a `<fighter>/<dir>` to take the MODEL from instead of the donor's.
        # Exactly one row uses it: Samus's charge shot has no model anywhere in the archive -- the
        # ball is made of her effects, which ADR 0002 excludes -- so cloning it faithfully gives a
        # projectile that hits and cannot be seen. It borrows Pikachu's own electro ball.
        #
        # GEOMETRY ONLY. The weapon is still WEAPON_KIND_SAMUS_CSHOT, so the flight, the hitboxes
        # and the straight line are hers; and her `motion_list.bin` declares two motions that name
        # no animation files at all, so nothing in the lent model has to line up with her motion
        # list. The MOTION side is never lent -- that is where the weapon's own behaviour lives.
        src_rel="$donor/$kind/$donor_name"
        if [ "$kind" = "model" ] && [ -n "${model_from:-}" ]; then
            src_rel="${model_from%%/*}/model/${model_from#*/}"
            echo "  ..  $name borrows its model from $src_rel (its donor has none)"
        fi
        base=$(find_src "$src_rel") || {
            # AN ARTICLE MAY LEGITIMATELY HAVE NO MODEL, and refusing to believe that is what
            # blocked Samus's charge shot. `fighter/samus/model/cshot` IS NOT A PATH IN THE ARCHIVE
            # -- only `motion/cshot/cNN/motion_list.bin` is -- because the ball she fires is made
            # entirely of effects. There is nothing to pull and nothing to ship.
            #
            # DISTINGUISHED FROM A MISSING PULL BY ASKING THE ARCHIVE, not by assuming. The 590,711
            # -path map is the same oracle gen-config.sh uses; with it present, "the archive has
            # zero paths under this directory" is a fact and the absence is expected. Without it
            # the old hard error stands, because then "absent locally" and "absent from the game"
            # are indistinguishable and guessing between them is how a real missing asset ships as
            # a silently incomplete article.
            arcmap="$ROOT/work/acquire/arc-paths.txt"
            if [ -s "$arcmap" ] && ! grep -q "^fighter/$src_rel/" "$arcmap"; then
                echo "  -   $src_rel: the archive has none, so there is none to ship"
                continue
            fi
            echo "  !! $src_rel is in neither work/arc-vis nor extracted --" >&2
            echo "     pull it with tools/arc-fetch.sh before enabling '$name'" >&2
            exit 1
        }
        # Resolve the slot list against what this donor actually has, so an explicit slot the
        # donor lacks is a hard error rather than a silently missing model.
        case "$ARTICLE_SLOTS" in
            all) want_slots=$(cd "$base" && ls -d c0* 2>/dev/null) ;;
            *)   want_slots="$ARTICLE_SLOTS" ;;
        esac
        for c in $want_slots; do
            [ -d "$base/$c" ] || { echo "  !! $src_rel has no $c" >&2; exit 1; }
        done
        # ONE COPY PER SLOT, not one per file. See the same note in tools/mirror-costumes.sh: a
        # process spawn costs about 100ms under Git Bash, so per-file `cp` made this phase 27s of
        # a 249s build even at one costume slot. The article directory was already cleared above,
        # so a directory copy needs no retire pass.
        slots=0
        for c in $want_slots; do
            srcdir="$base/$c"
            [ -d "$srcdir" ] || continue
            mkdir -p "$MOD/$kind/$name/$c"
            cp -r "$srcdir/." "$MOD/$kind/$name/$c/"
            n=$(find "$MOD/$kind/$name/$c" -maxdepth 1 -type f | wc -l)
            total=$((total+n))
            [ "$n" -gt 0 ] && slots=$((slots+1))
        done
        [ "$slots" -gt 0 ] || { echo "  !! $base has no populated cNN directory" >&2; exit 1; }
        echo "  shipped $kind/$name: $slots costume slot(s) from ${base#"$ROOT/"}"
    done
    echo "  -> article '$name': $total file(s)"
    shipped_any=1
done <<< "$rows"

[ "$shipped_any" = "0" ] && echo "  (no article assets shipped)"
exit 0
