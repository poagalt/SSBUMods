#!/usr/bin/env bash
# Runs a check only if anything it could possibly read has changed since it last passed.
#
# WHY. tools/check.sh takes 53 seconds and three steps are 50 of them: check-srcparams (10s),
# check-acmd-values (16s) and check-hitboxes (24s). All three re-derive the same answer from the
# same bytes on every deploy, and a deploy that only changed one status function pays the full
# cost. That is the tax on every iteration, so it is the thing worth removing.
#
# WHAT MAKES IT SAFE. The cache key is the CONTENT of everything the checks read -- every .rs in
# the plugin, every tool that produces the answer, the shipped motion_list, and the check script
# itself -- not a timestamp. tools/anim-dropin.sh rewrites all 74 animations on every build, so
# mtimes are worthless here; the same lesson the deploy manifest already learned the hard way.
# Hashing the lot is one sha1sum over a few megabytes.
#
# It records a key ONLY on success, so a failing check can never be cached into silence, and
# `CHECK_NOCACHE=1` (or tools/check.sh --all) re-runs everything regardless.
#
# usage:  bash tools/cached.sh <name> <command> [args...]
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NAME="$1"; shift

CACHE="$ROOT/work/.check-cache"
mkdir -p "$CACHE"

# Everything an answer can depend on. Deliberately generous: a file listed here that a given check
# does not read only costs an occasional needless re-run, while one that is missing costs a wrong
# pass -- and those are not remotely the same mistake.
#
# NOT `tools/*.sh` WHOLESALE. The first version hashed every script in tools/, which meant
# editing deploy.sh invalidated the hitbox check -- and since deploy.sh is edited while making
# deploys faster, the cache missed on exactly the runs it was added for. The key is the check's
# OWN script (whichever argument is a real file), plus the sources any of them can read.
key() {
    {
        # NOT buildstamp.rs. tools/gen-buildstamp.sh rewrites it with a fresh ISO timestamp on
        # every build, so including it meant the key changed every single time and the cache
        # never hit once in a real build-then-deploy cycle -- 53 seconds saved in isolation and
        # nothing saved in practice. It is a string constant the plugin logs; no check reads it.
        find "$ROOT/pikachu-moveset/plugin/src" -name '*.rs' -type f \
             ! -name 'buildstamp.rs' -print0 2>/dev/null
        find "$ROOT/tools/reach/src" "$ROOT/tools/motion-patch/src" "$ROOT/tools/prc-tool/src" \
             -name '*.rs' -type f -print0 2>/dev/null
        printf '%s\0' "$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/c00/motion_list.bin"
        # THE SHIPPED FK DUMPS, WHICH ARE THE PARITY HARNESS'S ACTUAL INPUT AND WERE MISSING.
        #
        # `hbtrace` reads `work/fk/pikachu_shipped_skel.json` for the skeleton and
        # `work/fk/shipped_<stem>.json` for each animation. Those ARE the OURS side of every hitbox
        # comparison. Neither was in the key, so a change to the shipped skeleton or to any shipped
        # animation left `check-hitboxes` printing "cached (nothing this check reads has changed
        # since it last passed)" when the most important thing it reads had changed.
        #
        # Found while baking Mythra's sword, which took the skeleton from 88 bones to 79 and back
        # and re-trimmed all 74 animations. The cache hit anyway. It turned out to be a legitimate
        # hit -- forced with CHECK_NOCACHE=1, parity came back OK at 38 moves and 460 hitbox-frames
        # -- but the key could not have known that, and "the check that proves the hitboxes are
        # right, skipped on a build that rewrote the skeleton" is not a risk worth 24 seconds.
        #
        # Only the SHIPPED dumps, not all of `work/fk/`: the source-fighter dumps are 140MB of game
        # data that cannot change within a session, while these are 19MB and hash in 0.17s.
        find "$ROOT/work/fk" -maxdepth 1 -type f \
             -name 'shipped_*.json' -print0 2>/dev/null
        find "$ROOT/work/fk" -maxdepth 1 -type f \
             -name 'pikachu_shipped_skel.json' -print0 2>/dev/null
        printf '%s\0' "$ROOT/tools/moves.tsv" "$ROOT/tools/scales.tsv" "$ROOT/tools/ported-motions.txt"
        for a in "$@"; do
            [ -f "$a" ] && printf '%s\0' "$a"
        done
    } | xargs -0 sha1sum 2>/dev/null | sha1sum | cut -d' ' -f1
}

K="$(key "$@")"
F="$CACHE/$NAME"

if [ "${CHECK_NOCACHE:-0}" != "1" ] && [ -f "$F" ] && [ "$(cat "$F")" = "$K" ]; then
    echo "  cached  (nothing this check reads has changed since it last passed)"
    exit 0
fi

"$@"
rc=$?
if [ "$rc" -eq 0 ]; then
    printf '%s' "$K" > "$F"
else
    rm -f "$F"
fi
exit "$rc"
