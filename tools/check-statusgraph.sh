#!/usr/bin/env bash
# Every new status must be fully registered, and nothing may transition into a status that is not.
#
# WHY. A new status kind is a number, and a number is exactly as good as the handlers behind it.
# smashline installs whatever is registered and leaves the rest of the table alone, so a status
# with a Main and no Pre runs the fighter's own Pre -- which is how the Game & Watch up-B ended up
# invisible, warping and spawning Quick Attack arrows: reusing 0x1E1 meant Pikachu's own Init and
# Exec still ran. And a status with a Pre and no Main is worse, because the engine calls into
# whatever the table happens to hold.
#
# Transitioning into an UNREGISTERED new id is the same failure from the other direction: Pikachu
# is sent to a status that does nothing, which looks like free-fall or a freeze rather than like a
# typo. That is what the up-B did for weeks.
#
# Neither of those is visible in a diff or a compile. Both are one grep.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
S=pikachu-moveset/plugin/src
rc=0

# Every RAT_STATUS_KIND_* the table declares, minus the ceiling marker.
declared=$(grep -oE 'pub const RAT_STATUS_KIND_[A-Z_0-9]+' "$S/specials/status.rs" \
           | sed 's/pub const //' | grep -v '_NUM$' | sort -u)

# 1. Pre and Main for every declared status.
#
# End is genuinely optional -- most of these have nothing to undo -- so it is reported as
# information rather than required.
missing=0
for k in $declared; do
    pre=$(grep -rE "\.status\(\s*Pre\s*,\s*$k\b" "$S" | wc -l)
    main=$(grep -rE "\.status\(\s*Main\s*,\s*$k\b" "$S" | wc -l)
    if [ "$pre" = 0 ] || [ "$main" = 0 ]; then
        printf "  FAIL %-38s Pre:%s Main:%s -- an unregistered half runs Pikachu's own handler\n" \
               "$k" "$pre" "$main"
        missing=$((missing+1))
        rc=1
    fi
done
[ "$missing" = 0 ] && echo "  ok  every declared status has both a Pre and a Main ($(echo "$declared" | wc -w) statuses)"

# 2. Nothing transitions into a RAT_ status that is not declared.
#
# A typo here compiles, because the constant would simply not exist -- so this catches the other
# case: a constant that exists in the table but whose handlers were never installed, reached by a
# change_status somewhere. Those two checks together mean a reachable status is a working one.
used=$(grep -rhoE 'RAT_STATUS_KIND_[A-Z_0-9]+' "$S" | grep -v '_NUM$' | sort -u)
orphans=0
for k in $used; do
    if ! echo "$declared" | grep -qx "$k"; then
        echo "  FAIL $k is referenced but not declared in specials/status.rs"
        orphans=$((orphans+1)); rc=1
        continue
    fi
    # Referenced in a transition, but with no handlers -- caught by check 1 already; this catches
    # the reverse, a declared-and-registered status nothing can ever reach.
done
[ "$orphans" = 0 ] && echo "  ok  every RAT_STATUS_KIND_* referenced in code is declared"

# 3. Every declared status is MENTIONED somewhere other than its own registration.
#
# THIS IS DELIBERATELY THE WEAK VERSION OF "IS IT REACHABLE". The first attempt looked for a
# `change_status` naming the constant, and it reported eight false positives -- every status this
# moveset enters is entered in a way that grep cannot follow:
#
#   change_status_request_from_script(      <- the constant is on the NEXT line
#       boma, RAT_STATUS_KIND_SHOULDER_START, false
#   );
#   let next = if band { LARIAT } else { SHOULDER }; fighter.change_status(next.into(), ..)
#   requested_throw() -> Option<i32>, then change_status(next.into())
#
# A check that cries wolf eight times out of eighteen is a check that gets ignored, and the
# reported "unreachable" statuses were all fine. So the question is narrowed to one grep can
# actually answer: does anything mention this status at all besides the line that registers it?
# That still catches the real mistake -- a status added to the table and wired up and then never
# used -- and it cannot produce a false alarm.
dead=""
for k in $declared; do
    n=$(grep -rn "\b$k\b" "$S" \
        | grep -v "/specials/status.rs:" \
        | grep -vE '\.status\(' \
        | grep -vcE ':\s*(//|///)' || true)
    [ "$n" = 0 ] && dead="$dead $k"
done
if [ -n "$dead" ]; then
    echo "  FAIL declared and registered but never used anywhere:$dead"
    rc=1
else
    echo "  ok  every declared status is used somewhere beyond its own registration"
fi

exit $rc
