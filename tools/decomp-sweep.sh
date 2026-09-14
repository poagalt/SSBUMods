#!/usr/bin/env bash
# Every module we hold, one after another, resumable, with a per-module heartbeat.
#
#   bash tools/decomp-sweep.sh
#
# `decomp-all.sh` does one module. This does all of them so the answer to "is it in the decomp"
# is yes by default rather than yes once somebody asks. Modules already complete are skipped in
# seconds, so re-running after adding an NRO costs only the new one.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
mkdir -p work/decomp
SWEEP=work/decomp/sweep.progress
stamp() { date -u +%Y-%m-%dT%H:%M:%SZ; }

mods=$(ls vendor/nro/prebuilt/nro/release/lua2cpp_*.nro | sed 's|.*lua2cpp_||; s|\.nro$||' | sort)
echo "[$(stamp)] sweep starting over: $(echo $mods | tr '\n' ' ')" | tee -a "$SWEEP"

for m in $mods; do
    if grep -qs "^\[.*\] $m: DONE" "work/decomp/$m.progress"; then
        echo "[$(stamp)] $m: already complete, skipping" | tee -a "$SWEEP"
        continue
    fi
    echo "[$(stamp)] $m: starting" | tee -a "$SWEEP"
    bash tools/decomp-all.sh "$m" 150 >> "work/decomp/$m.run.log" 2>&1
    echo "[$(stamp)] $m: $(tail -1 "work/decomp/$m.progress" 2>/dev/null)" | tee -a "$SWEEP"
done
echo "[$(stamp)] sweep DONE -- $(du -sh vendor/decomp 2>/dev/null | cut -f1) across $(ls vendor/decomp | wc -l) module(s)" | tee -a "$SWEEP"
