#!/usr/bin/env bash
# Check the VALUE of every lua_const this plugin uses against the game's own registration code.
#
#   bash tools/check-luaconst.sh            gate on the constants the plugin actually names
#   bash tools/check-luaconst.sh --all      also list every mismatch in the whole crate table
#
# WHY THIS EXISTS. Every `FIGHTER_KINETIC_TYPE_*`, `DAMAGE_LEVEL_*` and `*_STATUS_KIND_*` in this
# plugin takes its NUMBER from `skyline_smash`'s `lua_const.rs`, which is a hand-maintained
# third-party table of 38,743 `NAME: 0x...` pairs. `LuaConst` stores that literal directly --
# `pub struct LuaConst { value: i32 }` -- so a wrong row there is a wrong number compiled into the
# mod, and it looks exactly like correct code.
#
# `vendor/main/consts-1305.csv` is 39,213 `name,value` pairs recovered by EMULATING 13.0.5's own
# constant-registration functions (tools/emu-module.py consts). It is the game's answer, not
# anybody's transcription of it. So the two can be diffed, and "the crate says 3" becomes a
# verified fact instead of an assumption.
#
# This is the class of error that has already cost this project four wrong entries in a
# hand-written status-name table. "Never invent a number" stopped one level short of the numbers
# themselves; this closes that.
#
# A NAME MISSING FROM THE GAME TABLE IS NOT A FAILURE, and that is not laziness. `work/emu/
# consts.log` line 55 records the last registering function yielding 2,092 of an expected 2,897 --
# about 805 short, so the recovered set is ~98% of what that pass should have found. Absence is
# therefore evidence about the RECOVERY, not about the constant. Only a DISAGREEMENT is a finding.
#
# ONLY THE TOP-LEVEL `lua_consts!` INVOCATION IS COMPARED. `lua_const.rs` ends with ~40 more
# invocations nested in `pub mod AttackerAttribute { ... }` style blocks whose names are short and
# module-scoped -- `NONE: 0`, `ATTACK: 1`, `Normal: 0`. Those are not the game's global lua_const
# namespace, and folding them in would collide `NONE` against whatever the game registers under
# that name. The top-level entries are 4-space indented and have no duplicates; the nested ones
# are 8-space indented. That indent is the discriminator.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"

ALL=0
[ "${1:-}" = "--all" ] && ALL=1

GAME="${RAT_CONSTS:-$ROOT/vendor/main/consts-1305.csv}"

# The crate lives in cargo's git checkout cache, whose hash suffix is machine-specific, so it is
# found rather than hardcoded. RAT_SKYLINE_SMASH overrides for a vendored or patched copy.
CRATE="${RAT_SKYLINE_SMASH:-}"
if [ -z "$CRATE" ]; then
    for c in "$HOME"/.cargo/git/checkouts/skyline-smash-*/*/src/lua_const.rs; do
        [ -f "$c" ] && CRATE="$c"
    done
fi

[ -s "$GAME" ]  || { echo "no game constant table: $GAME" >&2; exit 1; }
[ -s "$CRATE" ] || { echo "no skyline-smash lua_const.rs found; set RAT_SKYLINE_SMASH" >&2; exit 1; }

W="$ROOT/work/luaconst"; mkdir -p "$W"

# `tr -d '\r'` on everything read. Every file in work/emu/ is CRLF and a bare `$` anchor silently
# matches nothing against one, which reads exactly like "no rows" rather than like a parse failure.
tr -d '\r' < "$CRATE" \
  | awk 'match($0, /^    ([A-Za-z_][A-Za-z0-9_]*):[[:space:]]*(0x[0-9A-Fa-f]+|-?[0-9]+),?[[:space:]]*$/, m) {
        printf "%s\t%d\n", m[1], strtonum(m[2])
    }' | sort -u > "$W/crate.tsv"

tr -d '\r' < "$GAME" \
  | awk -F, '!/^#/ && NF>=4 {
        name = $3; gsub(/^"|"$/, "", name)
        if (name != "") printf "%s\t%d\n", name, strtonum($4)
    }' | sort -u > "$W/game.tsv"

# ASSERT NON-EMPTY RATHER THAN TRUST A ZERO. A silently-empty side would make every comparison
# below vacuous and the script would print a clean bill of health for having read nothing.
nc=$(wc -l < "$W/crate.tsv"); ng=$(wc -l < "$W/game.tsv")
[ "$nc" -gt 0 ] || { echo "parsed 0 constants out of $CRATE -- format changed?" >&2; exit 1; }
[ "$ng" -gt 0 ] || { echo "parsed 0 constants out of $GAME -- format changed?" >&2; exit 1; }
echo "  crate: $nc constant(s)   game: $ng constant(s)"

# The constants the plugin ACTUALLY names. 38,743 rows is far more than this mod touches, and a
# disagreement on a constant nothing references cannot change a compiled number. Intersecting the
# crate's names with the uppercase identifiers in the plugin source is both exact enough and fast:
# `use smash::lua_const::*` means a used constant appears verbatim as a bare word.
grep -rhoE '\b[A-Z][A-Z0-9_]{3,}\b' pikachu-moveset/plugin/src 2>/dev/null \
  | sort -u > "$W/plugin-idents.txt"
cut -f1 "$W/crate.tsv" | sort -u > "$W/crate-names.txt"
comm -12 "$W/crate-names.txt" "$W/plugin-idents.txt" > "$W/used.txt"
nu=$(wc -l < "$W/used.txt")
echo "  of those, $nu are named by the plugin"

# The join. Three outcomes, and only the first is a failure.
join -t$'\t' "$W/crate.tsv" "$W/game.tsv" \
  | awk -F'\t' '$2 != $3 { printf "%s\tcrate=0x%X\tgame=0x%X\n", $1, $2, $3 }' \
  | sort > "$W/mismatch.tsv"
join -t$'\t' -v1 "$W/crate.tsv" "$W/game.tsv" | cut -f1 > "$W/crate-only.txt"
join -t$'\t' -v2 "$W/crate.tsv" "$W/game.tsv" | cut -f1 > "$W/game-only.txt"

nm=$(wc -l < "$W/mismatch.tsv")
ncmp=$(join -t$'\t' "$W/crate.tsv" "$W/game.tsv" | wc -l)
echo "  compared $ncmp name(s) present in both"

rc=0

# Mismatches among the constants the plugin uses. THIS is the gate.
awk -F'\t' 'NR==FNR { U[$1]=1; next } ($1 in U)' "$W/used.txt" "$W/mismatch.tsv" > "$W/mismatch-used.tsv"
nmu=$(wc -l < "$W/mismatch-used.tsv")
if [ "$nmu" -gt 0 ]; then
    echo "  FAIL $nmu constant(s) the plugin uses disagree with the game:"
    sed 's/^/       /' "$W/mismatch-used.tsv"
    rc=1
else
    echo "  ok  every constant the plugin names agrees with the game's own registration"
fi

# Mismatches anywhere else in the table are reported but do not gate: they say the crate table is
# imperfect, which is worth knowing and is not this mod's bug.
if [ "$nm" -gt "$nmu" ]; then
    echo "  note $((nm - nmu)) further mismatch(es) elsewhere in the crate table (not used here)"
    if [ "$ALL" -eq 1 ]; then sed 's/^/       /' "$W/mismatch.tsv"
    else echo "       run with --all to list them"; fi
fi

# Coverage notes. Neither is a failure -- see the header on the 98% recovery.
noc=$(wc -l < "$W/crate-only.txt"); nog=$(wc -l < "$W/game-only.txt")
echo "  note $noc crate constant(s) absent from the recovered table (~98% complete, so absence proves nothing)"

# THE GAME-ONLY COUNT IS SPLIT, BECAUSE THE RAW NUMBER MISREADS BY 6x. Most of what the game
# registers and the top-level table lacks IS in the crate -- in one of the nested `pub mod`
# invocations this comparison deliberately skips, reachable as `AttackerAttribute::NONE` rather
# than as a bare word. Reporting a flat "473 the crate does not name" invites exactly the wrong
# conclusion about the crate's coverage; 406 of those 473 are `pub mod` entries and 67 are real
# gaps. A number that is load-bearing should say which number it is.
tr -d '\r' < "$CRATE" \
  | awk 'match($0, /^        ([A-Za-z_][A-Za-z0-9_]*):/, m) { print m[1] }' \
  | sort -u > "$W/nested-names.txt"
nnest=$(comm -12 "$W/game-only.txt" "$W/nested-names.txt" | wc -l)
comm -13 "$W/nested-names.txt" "$W/game-only.txt" > "$W/game-only-real.txt"
nreal=$(wc -l < "$W/game-only-real.txt")
echo "  note $nog constant(s) the game registers that the TOP-LEVEL table does not name:"
echo "       $nnest of them are in a nested \`pub mod\` block this check skips by design"
echo "       $nreal are genuinely absent from the crate (see work/luaconst/game-only-real.txt)"
awk -F'\t' 'NR==FNR { U[$1]=1; next } ($1 in U)' "$W/used.txt" "$W/crate-only.txt" > "$W/used-unverified.txt" 2>/dev/null || true
nuu=$(wc -l < "$W/used-unverified.txt" 2>/dev/null || echo 0)
[ "$nuu" -gt 0 ] && echo "  note $nuu constant(s) the plugin uses could NOT be verified either way"

exit $rc
