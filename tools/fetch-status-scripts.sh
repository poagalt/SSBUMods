#!/usr/bin/env bash
# Every fighter's decompiled status scripts, from the upstream dump.
#
#   bash tools/fetch-status-scripts.sh
#
# `vendor/ssbu-status` is a FIVE-FIGHTER copy of this, committed into the repo back when five was
# all the moveset touched. That limit is what made "how does Donkey Kong enter the carry" look
# unanswerable for five rounds -- the answer needed a fighter nobody had checked out and, as it
# turned out, `lua2cpp_common.nro`, which this dump does not contain at all.
#
# So: all 95 fighters here, `common` from `tools/decomp-all.sh common`, and between them there is
# no fighter status function in the game that cannot be read.
#
# Gitignored -- 124 MB, third-party, and one clone away.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
DEST=vendor/ssbu-status-all
UPSTREAM=https://github.com/Coolsonickirby/SSBU-Dumped-Status-Scripts
if [ -d "$DEST/.git" ]; then
    echo "updating $DEST"; git -C "$DEST" pull --ff-only
else
    echo "cloning $UPSTREAM into $DEST"; git clone --depth 1 "$UPSTREAM" "$DEST"
fi
echo "  $(ls "$DEST/ghidra_scripts" | wc -l) fighter module(s)"
