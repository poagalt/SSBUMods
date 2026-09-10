#!/usr/bin/env bash
# The hand-written specials must use the SOURCE FIGHTER'S OWN PARAM VALUES, and must say which
# ones they are not using.
#
# The generated ACMDs are copied verbatim from the dumps and tools/check-acmd-values.sh proves it.
# The status modules are different: Alolan Whip, Revenge, the cargo carry and the up-B are written
# by hand, and every number in them was read out of the source fighter's vl.prc once and typed in.
# Nothing checked that they still match, and nothing at all reported the params that were never
# typed in -- which is where missing BEHAVIOUR hides, not just wrong numbers.
#
# Two questions, and the second is the one with no previous answer:
#
#   1. For every source param the module NAMES, does the game's value still appear in the module?
#      A drift check. Fails.
#   2. Which params of that object does the module never name at all? Each one is a piece of the
#      source move that is either deliberately not reproduced or accidentally missing, and the
#      difference should be written down rather than remembered. Reports.
#
# The value match is deliberately loose -- the number has to appear somewhere in the module, not in
# a particular place -- because the modules legitimately transform them: `special_lw_revenge_time`
# is 60 SECONDS in the prc and `60 * 60` frames in the code, and the lariat offsets are negative in
# the prc because Incineroar's are behind him. It catches 0.075 becoming 0.75, which is the failure
# that actually happens, and does not pretend to check semantics.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
DUMP=tools/prc-tool/target/release/prcdump.exe
LABELS=vendor/param-labels/ParamLabels.csv

# fighter | param object | module directory | what it drives
TABLE="
gaogaen|param_special_s|specials|Alolan Whip
gaogaen|param_special_lw|speciallw|Revenge
gamewatch|param_special_hi|specialhi|up-B Fire
donkey|param_lifting_walk|cargo|cargo carry
"

fail=0
total_named=0
total_unnamed=0
unnamed_list=""

while IFS='|' read -r fighter obj dir what; do
    [ -z "${fighter// }" ] && continue
    src="extracted/$fighter/param/vl.prc"
    [ -f "$src" ] || src="extracted/fighter/$fighter/param/vl.prc"
    if [ ! -f "$src" ]; then
        echo "  FAIL no vl.prc for $fighter"; fail=1; continue
    fi
    mod="pikachu-moveset/plugin/src/specials/$dir"
    if [ ! -d "$mod" ]; then
        echo "  FAIL no module at $mod"; fail=1; continue
    fi
    body=$(cat "$mod"/*.rs 2>/dev/null)

    # TWO SHAPES, and assuming one of them reported a whole object as missing.
    #
    # Most special params are a `list[4]` whose [0] entry is the real struct and whose [1..3] are
    # zeroed per-slot copies -- reading those would report every param twice, once with a value of
    # 0. But `param_lifting_walk` is a bare `struct[3]` with the values one level up and no [0] at
    # all, so the list-shaped reader found nothing and this failed with "name or layout changed"
    # on an object that was perfectly fine.
    all=$("$DUMP" "$src" "$LABELS" 2>/dev/null \
        | awk -v want="  $obj " '
            index($0, want) == 1 { inobj = 1; print; next }
            inobj && /^  [a-z0-9_]+ / { inobj = 0 }
            inobj { print }
          ')
    params=$(printf '%s' "$all" | awk '
        /^    \[0\]/ { instruct = 1; next }
        /^    \[[1-9]\]/ { instruct = 0 }
        instruct && /^      [a-z0-9_]+ = / { print }
      ')
    # Bare struct: the values sit at one less indentation and there is no [0] to enter.
    if [ -z "$params" ]; then
        params=$(printf '%s' "$all" | awk '/^    [a-z0-9_]+ = / { print }')
    fi
    if [ -z "$params" ]; then
        echo "  FAIL $fighter $obj yielded no params -- name or layout changed"; fail=1; continue
    fi

    named=0; unnamed=0; declined=0; missing=""; declined_list=""
    while read -r name eq value; do
        [ -z "${name:-}" ] && continue
        # THREE BUCKETS, BECAUSE TWO MADE THIS GAMEABLE BY PROSE.
        #
        # With only "mentioned" and "not mentioned", writing a comment that says a param is NOT
        # modelled moved it into the covered column -- the check rewarded explaining the gap by
        # hiding it. `NOT MODELLED: <param>` is now its own answer: declared, reported, and not
        # counted as reproduced. Anything mentioned any other way is claimed as used and has its
        # value checked.
        if printf '%s' "$body" | grep -qE "NOT MODELLED:.*\b$name\b"; then
            declined=$((declined+1)); declined_list="$declined_list $name"; continue
        fi
        if ! printf '%s' "$body" | grep -q "\b$name\b"; then
            unnamed=$((unnamed+1)); missing="$missing $name=$value"; continue
        fi
        named=$((named+1))
        # The prc prints integers without a decimal point; Rust f32 literals carry one. Accept
        # either spelling, plus the bare decimal for fractional values.
        if printf '%s' "$body" | grep -qE "(^|[^0-9.])$(printf '%s' "$value" | sed 's/[.]/\\./g')([^0-9]|$)" \
           || printf '%s' "$body" | grep -qF "$value.0"; then
            continue
        fi
        echo "  FAIL $what: $name is $value in $fighter's vl.prc but that value is not in $dir/"
        fail=1
    done <<< "$params"

    total_named=$((total_named+named))
    total_unnamed=$((total_unnamed+unnamed))
    printf "  %-14s %-24s %2d used, %2d declared unmodelled, %2d unaccounted\n" \
           "$what" "$fighter $obj" "$named" "$declined" "$unnamed"
    [ -n "$declined_list" ] && echo "                 not modelled:$declined_list"
    [ -n "$missing" ] && unnamed_list="$unnamed_list
      $what:$missing"
done <<< "$TABLE"

if [ "$fail" -eq 0 ]; then
    echo "  ok  $total_named source param(s) named and matching"
fi
# UNACCOUNTED IS NOW A FAILURE, because the baseline is zero.
#
# Every one of the 83 source params across the four hand-written specials is either used with its
# value checked, or carries a `NOT MODELLED:` line saying why not. Holding that at zero means a
# param nobody has looked at cannot slip in silently -- the cost of clearing it is one honest
# sentence, which is exactly the artifact that was missing.
if [ "$total_unnamed" -gt 0 ]; then
    echo "  FAIL $total_unnamed source param(s) the modules neither use nor decline:$unnamed_list"
    echo "       Add a \`// NOT MODELLED: <param>\` line saying why, or use the value."
    fail=1
fi
exit "$fail"
