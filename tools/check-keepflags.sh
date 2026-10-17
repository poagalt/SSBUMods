#!/usr/bin/env bash
# Every source status flag the STATUS code reads must survive `deforeign`, or the ACMD that sets it
# is being deleted out from under it.
#
# WHY THIS EXISTS. `deforeign` strips statements referencing a source fighter's namespace constants,
# because those are per-fighter enumerations that collide on the host -- that is the ADR 0006
# problem and the stripping is correct. It keeps an explicit allowlist, `KEEP_STATUS_IDS`, for
# status-scoped ids that are safe because the moveset owns the whole status.
#
# That allowlist has to be kept in step BY HAND with what the hand-written status modules read, and
# it went stale in the worst possible direction: `FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_MOVE_START`
# and `_FLAG_MOVE_END` are raised by `game_specialsstart` on frames 1 and 39 and were being
# stripped, while `whip_move_flags` in `src/specials/whip/` read them to start and stop the lunge.
# Result: the whip played its full animation standing still, and because the catch box only reaches
# 10.94 units while the lunge is what closes the distance, it also never grabbed. Three playtest
# rounds went into that.
#
# IT WAS INVISIBLE TO EVERY EXISTING CHECK. The strip removes the statement and leaves the
# `frame()` marker, so the generated file grows an empty `frame(39.0);` that reads as deliberate.
# `check-acmd-values.sh` compares macro ARGUMENTS and a bare `WorkModule::on_flag` has none.
# `check-nativecalls.sh` looks at engine calls in status functions, not ACMD statements. Nothing
# was watching the seam between the two halves.
#
# THE RULE. For every `FIGHTER_<SRC>_STATUS_*` id referenced by a hand-written status module, if any
# ported source ACMD writes that id, then the id must be in `deforeign`'s KEEP_STATUS_IDS. A read
# with no surviving write is a half-connected mechanism, and that is exactly what this catches.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"

DEF=tools/reach/src/bin/deforeign.rs
D=vendor/SSBU-Dumped-Scripts/smashline
S=pikachu-moveset/plugin/src

# The generated modules are deforeign's output; everything else under src/specials is hand-written
# status code. Only the hand-written side counts as a "reader" -- a reference inside a generated
# module is the very thing being stripped.
GENERATED='normals|smashes|aerials|throws|defensive|landings|dodges|specials_acmd'

fail=0
checked=0

# Every source status id the hand-written status modules mention.
#
# COMMENTS ARE STRIPPED FIRST. This project's modules explain themselves at length and those
# explanations name constants -- a prose mention of "the `FIGHTER_GAOGAEN_STATUS_SPECIAL_S_*`
# family" matched as a truncated id `FIGHTER_GAOGAEN_STATUS_SPECIAL_S_` and was reported as a
# missing flag. `check-nativecalls.sh` learned the same lesson: a check that counts doc comments as
# code is gameable by prose in one direction and noisy in the other.
reads=$(grep -rh --include='*.rs' -v -E '^\s*(//|///)' "$S/specials" "$S/sword" "$S/articles" "$S/victim.rs" 2>/dev/null \
        | grep -oE 'FIGHTER_[A-Z0-9]+_STATUS_[A-Z_0-9]*[A-Z0-9]' \
        | grep -vE "^FIGHTER_(STATUS|INSTANCE)_" | sort -u)

# THE WRITE SET, BUILT IN ONE PASS PER FIGHTER RATHER THAN ONE GREP PER ID.
#
# `$D` is 38,111 files. The first version of this loop ran a RECURSIVE grep over the WHOLE tree
# for every id it checked -- twenty full walks of 38,111 files -- and on Git Bash under Windows,
# where a bare `find` over that tree takes over a minute, it was 780 of this harness's 780
# seconds. The per-gate clock tools/check.sh now prints is what made that visible at all.
#
# Two changes, and both make the answer better as well as faster:
#
#   * SCOPED TO THE ID'S OWN FIGHTER. An id names its namespace -- `FIGHTER_GAOGAEN_STATUS_*` is
#     gaogaen's -- and the question here is whether the ACMD ported ALONGSIDE that status writes
#     it. A write of gaogaen's id from an unrelated fighter's dump (Kirby's copy scripts do
#     exactly that) was counted as evidence before and is not evidence at all.
#   * ONE PASS PER FIGHTER, NOT PER ID. Several ids share a namespace, so the tree is walked once
#     for each DISTINCT fighter that appears among them -- three or four, not twenty.
#
# The set is every `FIGHTER_<NS>_STATUS_*` identifier appearing inside a write call. `grep -o`
# with the call name and the id in one pattern keeps the two together, so a flag merely READ by
# the source's own ACMD does not enter the set.
want_ns=""
for id in $reads; do
    ns=$(printf '%s' "$id" | sed -E 's/^FIGHTER_([A-Z0-9]+)_STATUS_.*/\1/')
    grep -q "\"$ns\"" "$DEF" || continue
    case "$id" in *_STATUS_KIND_*) continue ;; esac
    case " $want_ns " in *" $ns "*) ;; *) want_ns="$want_ns $ns" ;; esac
done

WRITES=$(mktemp); trap 'rm -f "$WRITES"' EXIT
for ns in $want_ns; do
    lns=$(printf '%s' "$ns" | tr 'A-Z' 'a-z')
    fdir="$D/lua2cpp_$lns"
    [ -d "$fdir" ] || continue
    grep -rhoE "(on_flag|off_flag|set_int|set_int64|set_float)\([^)]*\*?FIGHTER_${ns}_STATUS_[A-Z_0-9]*" "$fdir" 2>/dev/null \
      | grep -oE "FIGHTER_${ns}_STATUS_[A-Z_0-9]*" >> "$WRITES"
done
sort -u -o "$WRITES" "$WRITES"

for id in $reads; do
    # Engine-global ids (FIGHTER_STATUS_*, FIGHTER_STATUS_KIND_*) are not foreign; skip anything
    # whose namespace is not one of deforeign's SOURCES.
    ns=$(printf '%s' "$id" | sed -E 's/^FIGHTER_([A-Z0-9]+)_STATUS_.*/\1/')
    grep -q "\"$ns\"" "$DEF" || continue
    # A status KIND is a plain integer compared against, not a work id written by an ACMD.
    case "$id" in *_STATUS_KIND_*) continue ;; esac

    # Does the source's own ACMD actually write it? If not, the status code is the only party and
    # there is nothing for deforeign to break.
    #
    # THE BOUNDARY MATTERS, AND `-x` IS WHAT ENFORCES IT. `FLAG_MOVE` is a prefix of
    # `FLAG_MOVE_START`, so a substring test reported `FLAG_MOVE` as ACMD-written when the only
    # thing the ACMD writes is `FLAG_MOVE_START`. `FLAG_MOVE` is in fact written by
    # `whip_move_flags` in the status code, which is not stripped and needs no allowlist entry.
    # A whole-line fixed match against the extracted set keeps the two apart, the same way the
    # old `[^A-Z_0-9]` terminator did.
    grep -qxF "$id" "$WRITES" || continue

    checked=$((checked+1))
    if ! grep -q "\"$id\"" "$DEF"; then
        echo "  FAIL $id"
        echo "       read by hand-written status code, WRITTEN by a ported source ACMD, and not in"
        echo "       deforeign's KEEP_STATUS_IDS -- so the write is stripped and the read sees a"
        echo "       flag that is never set. Add it to KEEP_STATUS_IDS after the three-point safety"
        echo "       check in $DEF, or stop reading it."
        fail=1
    fi
done

if [ "$fail" -eq 0 ]; then
    echo "  ok  $checked source status flag(s) read by status code survive deforeign"
fi
exit $fail
