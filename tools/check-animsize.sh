#!/usr/bin/env bash
# The shipped animation directory must not be much bigger than the one it replaces.
#
# WHY THIS IS A CHECK AND NOT A PREFERENCE. A fighter's loaded animation set has a memory budget,
# and the in-game self-test caught the port exceeding it: 25 of the 30 added files reported
# UNRESOLVED, nondeterministically, DIFFERENTLY between two loads of the same build -- and one
# round earlier the same build had reported 77 of 77 fine. Identical files cannot fail
# differently, so the variable was how much else was resident.
#
# The cause was dead weight, not the port being large. A dropped-in animation is the SOURCE
# fighter's file and carries a track per source bone and per source mesh: Incineroar's
# `d01specialsstart` had 123 Transform nodes against Pikachu's 80 bones, of which it names 46, so
# 77 tracks animated nothing. It shipped at 7.16x the file it replaced. tools/anim-dropin.sh now
# drops any track whose target does not exist, which took the whole directory from +3673 KiB over
# vanilla to +637 KiB and did not move a single hitbox -- tools/check-hitboxes.sh reports 452
# hitbox-frames all within 0.05 world units of the source, before and after.
#
# So the budget is enforced rather than trusted, because the failure it prevents does not look
# like a size problem. It looks like a move that works on Tuesday.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
V="extracted/fighter/pikachu/motion/body/c00"
M="pikachu-moveset/mod/fighter/pikachu/motion/body/c00"

# Headroom over vanilla, in KiB. 1024 is comfortably above the 637 the trimmed port needs and
# comfortably below the 3673 that broke it. A legitimate reason to raise it would be more ported
# moves, and raising it should be a deliberate edit with a measurement attached.
BUDGET_KIB=1024
# And no single replacement may be more than this many times the file it replaces. Catches one
# enormous animation slipping in even while the total still fits.
WORST_RATIO=300   # 3.00x, in hundredths -- bash has no floats

[ -d "$V" ] || { echo "  skip no extracted vanilla pikachu to compare against"; exit 0; }
[ -d "$M" ] || { echo "  skip nothing shipped yet"; exit 0; }

van=0; ours=0; rc=0; worst=0; worstname=""
for f in "$M"/*.nuanmb; do
    b=$(basename "$f")
    ms=$(stat -c%s "$f")
    ours=$((ours + ms))
    if [ -f "$V/$b" ]; then
        vs=$(stat -c%s "$V/$b")
        van=$((van + vs))
        [ "$vs" -gt 0 ] || continue
        r=$(( ms * 100 / vs ))
        if [ "$r" -gt "$worst" ]; then worst=$r; worstname=$b; fi
        if [ "$r" -gt "$WORST_RATIO" ]; then
            printf "  FAIL %-30s is %d.%02dx the vanilla file it replaces (max %d.%02dx)\n" \
                   "$b" $((r/100)) $((r%100)) $((WORST_RATIO/100)) $((WORST_RATIO%100))
            rc=1
        fi
    fi
done

growth=$(( (ours - van) / 1024 ))
if [ "$growth" -gt "$BUDGET_KIB" ]; then
    echo "  FAIL the animation directory is ${growth} KiB over vanilla; the budget is ${BUDGET_KIB} KiB."
    echo "       Over budget, added animations stop resolving nondeterministically -- the in-game"
    echo "       self-test reports them UNRESOLVED and moves silently play the wrong pose."
    rc=1
elif [ "$rc" = 0 ]; then
    printf "  ok  animation directory is %d KiB over vanilla (budget %d); worst replacement %d.%02dx (%s)\n" \
           "$growth" "$BUDGET_KIB" $((worst/100)) $((worst%100)) "$worstname"
fi
exit $rc
