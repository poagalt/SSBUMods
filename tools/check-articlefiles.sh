#!/usr/bin/env bash
# A cloned article must ship EVERY file the archive has for it, not just the ones we happen to
# hold locally.
#
# THIS IS THE CRASH OF 2026-09-20, AND IT IS A HOLE IN THE CHECK NEXT DOOR. Byleth's bow was
# enabled, the build shipped it, every gate passed, and the game took a Data Abort at fault
# address 0 inside the engine's fighter object initialisation -- `smashline + 0x2a68c
# (fighter_initialize_object_data + 0x2c)`, which is just the trampoline into `call_original!`, so
# the null deref is the game's own article construction.
#
# The archive has EIGHTEEN files for `fighter/master/model/bow/c00`. `extracted/` had ONE.
# `tools/gen-article-assets.sh` searches `work/arc-vis` first and falls back to `extracted/`, and
# the bow has no `arc-vis` copy -- so it shipped a model directory containing a single file, and a
# model that is one file is a null pointer waiting for fighter load.
#
# WHY check.sh's 3f DID NOT CATCH IT. That gate asserts the shipped bytes are byte-identical to
# the source they were copied from, which they were. It compares the copy against the same partial
# source, so a partial source is invisible to it by construction. The two working articles happen
# to be complete (cloud/fusionsword 14 of 14, gamewatch/parachute 16 of 16) because they were
# pulled off the running console into `work/arc-vis`, and nothing said out loud that that was the
# reason they worked.
#
# THE ORACLE IS ALREADY HERE. `work/acquire/arc-paths.txt` is all 590,711 paths in the archive --
# the same file gen-config.sh uses to decide what is a new path, and the same one gen-article-
# assets.sh now asks before believing an article legitimately has no model at all. Counting its
# entries for `fighter/<donor>/<kind>/<article>/<slot>/` is the completeness test, and it needs no
# console.
#
# WITHOUT THE MAP THIS CANNOT JUDGE, and says so rather than passing. "absent locally" and "absent
# from the game" are indistinguishable without it, which is the same distinction the generator has
# to make.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"

A="pikachu-moveset/plugin/src/articles/mod.rs"
MOD="pikachu-moveset/mod/fighter/pikachu"
ARCMAP="work/acquire/arc-paths.txt"
rc=0

if [ ! -s "$ARCMAP" ]; then
    echo "  !!  no $ARCMAP -- article completeness is UNCHECKED. Pull it before a release;"
    echo "      this is the check that would have caught the bow shipping 1 of its 18 model files."
    exit 0
fi

clone_on=$(grep -cx 'pub const CLONE_ENABLED: bool = true;' "$A" || true)
if [ "$clone_on" = "0" ]; then
    echo "  -   CLONE_ENABLED=false, no article ships assets"
    exit 0
fi

# host | clean | enabled | authored | donor | donor_article | model_from
rows=$(sed -n 's/.*Candidate *{ *host_name: *"\([^"]*\)".*donor: *"\([^"]*\)".*donor_article: *"\([^"]*\)".*clean: *\([a-z]*\) *, *enabled: *\([a-z]*\) *, *authored: *\([a-z]*\) *, *model_from: *"\([^"]*\)".*/\1|\4|\5|\6|\2|\3|\7/p' "$A")

checked=0
while IFS='|' read -r host clean enabled authored donor dart model_from; do
    [ -z "${host:-}" ] && continue
    [ "$enabled" = "true" ] && [ "$clean" = "true" ] || continue
    # AN AUTHORED ARTICLE HAS NO ARCHIVE COPY TO BE COMPLETE AGAINST. Its model is assembled from
    # the donor's BODY model by tools/author-article.py, so counting arc paths for it is asking
    # the wrong question -- tools/check.sh's 3f2 checks those against the donor's geometry instead.
    if [ "$authored" = "true" ]; then
        echo "  -   $host is authored; its geometry is checked by 3f2, not by file count"
        continue
    fi
    for kind in model motion; do
        d="$MOD/$kind/$host"
        [ -d "$d" ] || continue
        for slotdir in "$d"/c*; do
            [ -d "$slotdir" ] || continue
            slot=$(basename "$slotdir")
            # A LENT MODEL IS COUNTED AGAINST WHAT IT WAS LENT FROM. `model_from` names a
            # `<fighter>/<dir>` whose model stands in for a donor that has none -- Samus's charge
            # shot borrowing Pikachu's electro ball -- so that is the archive path to count.
            # Counting the donor's would report "the archive has none", which is true and is the
            # very reason the substitution exists.
            src="$donor/$kind/$dart"
            if [ "$kind" = "model" ] && [ -n "${model_from:-}" ]; then
                src="${model_from%%/*}/model/${model_from#*/}"
            fi
            want=$(grep -c "^fighter/$src/$slot/" "$ARCMAP")
            got=$(find "$slotdir" -type f | wc -l)
            checked=$((checked+1))
            if [ "$want" = "0" ]; then
                # The generator skips a directory the archive has none of; reaching here means it
                # shipped one anyway, which is its own kind of wrong.
                echo "  FAIL $kind/$host/$slot ships $got file(s) but the archive has no"
                echo "       fighter/$src/$slot at all"
                rc=1
            elif [ "$got" -lt "$want" ]; then
                echo "  FAIL $kind/$host/$slot ships $got of the archive's $want file(s) --"
                echo "       an incomplete article model is a null dereference at fighter load."
                echo "       Pull the rest with: arc-fetch the paths under fighter/$src/"
                rc=1
            else
                echo "  ok  $kind/$host/$slot ships $got of $want file(s)"
            fi
        done
    done
done <<< "$rows"

# ---- AND A SHIPPED MODEL MUST ACTUALLY CONTAIN A MODEL ------------------------------------------
#
# COMPLETE IS NOT THE SAME AS NON-EMPTY, and the difference stalled a training-arena load on
# 2026-09-20. Samus's charge shot was lent Pikachu's electro ball to give it something visible;
# `fighter/pikachu/model/dengekidama` has all eleven of its archive paths, so the count above was
# happy -- and every one of them is a shell. A 40-byte `model.numatb` with ZERO material entries
# and a 144-byte `model.numdlb` with ZERO mesh rows. His jolt is a pure-effect article, exactly
# like her shot, and handing the engine an empty mesh to build a weapon from hung the load.
#
# So the count is checked against the archive AND the content is checked against itself. One mesh
# object and one material is the floor; a real projectile clears it by a mile (her supermissile
# has a 24,720-byte numshb) and a stub cannot clear it at all.
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
if [ -x "$T" ]; then
    W=$(mktemp -d); trap 'rm -rf "$W"' EXIT
    while IFS='|' read -r host clean enabled authored donor dart model_from; do
        [ -z "${host:-}" ] && continue
        [ "$enabled" = "true" ] && [ "$clean" = "true" ] || continue
        [ "$authored" = "true" ] && continue
        for slotdir in "$MOD/model/$host"/c*; do
            [ -d "$slotdir" ] || continue
            slot=$(basename "$slotdir")
            [ -f "$slotdir/model.numshb" ] || continue
            nm=0; nmat=0
            "$T" "$slotdir/model.numshb" "$W/s.json" >/dev/null 2>&1                 && nm=$(jq -r '.objects|length' "$W/s.json" 2>/dev/null | tr -d '')
            "$T" "$slotdir/model.numatb" "$W/m.json" >/dev/null 2>&1                 && nmat=$(jq -r '.entries|length' "$W/m.json" 2>/dev/null | tr -d '')
            if [ "${nm:-0}" = "0" ] || [ "${nmat:-0}" = "0" ]; then
                echo "  FAIL model/$host/$slot is a STUB: ${nm:-0} mesh object(s), ${nmat:-0} material(s)."
                echo "       Every file the archive has is present, and there is still no model in"
                echo "       them -- that donor's article is pure effect. Lend a real one."
                rc=1
            else
                echo "  ok  model/$host/$slot has ${nm} mesh object(s) and ${nmat} material(s)"
            fi
        done
    done <<< "$rows"
fi

if [ "$checked" = "0" ]; then
    echo "  -   no installed article ships assets to check"
elif [ "$rc" = "0" ]; then
    echo "  ok  every installed article ships its whole archive directory ($checked checked)"
fi
exit $rc
