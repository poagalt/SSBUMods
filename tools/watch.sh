#!/usr/bin/env bash
# Live playtest watcher. Streams the plugin's telemetry off the console and resolves the
# raw hash40 motion ids to names using the local dictionary.
#
#   bash tools/watch.sh            # follow live
#   bash tools/watch.sh 300        # follow for 300 seconds then stop
#
# Works while the game is running, which is when FTP is unavailable.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
. "$ROOT/tools/console.sh"
IP=$CONSOLE_HOST
SECS="${1:-1800}"
LOG="$ROOT/work/playtest-$(date +%H%M%S).log"
mkdir -p "$ROOT/work"

# name -> hash lookup table, built once
python - >/dev/null 2>&1 <<'PY' || true
PY
awk -F, 'NF>1 {print $1","$2}' "$ROOT/vendor/param-labels/ParamLabels.csv" > /tmp/_names.csv 2>/dev/null

export PATH="/c/Users/poaga/.cargo/bin:$PATH"
echo "watching $IP for ${SECS}s -> $LOG"
echo "(motion ids are hash40; resolved below where known)"
cd "$ROOT/pikachu-moveset/plugin"
timeout "$SECS" cargo skyline listen --ip "$IP" 2>&1 | tee "$LOG" | grep --line-buffered "\[rat\]" | while read -r line; do
    h=$(printf '%s' "$line" | grep -oE '0x[0-9a-f]{12}' | head -1)
    if [ -n "$h" ]; then
        short=$(printf '%s' "$h" | sed 's/^0x0*/0x/')
        name=$(grep -m1 -i ",${short}\$\|^${short}," /tmp/_names.csv 2>/dev/null | cut -d, -f2)
        [ -z "$name" ] && name=$(grep -m1 "^0*${short#0x}," "$ROOT/vendor/param-labels/ParamLabels.csv" 2>/dev/null | cut -d, -f2)
        if [ -n "$name" ]; then printf '%s   <- %s\n' "$line" "$name"; else echo "$line"; fi
    else
        echo "$line"
    fi
done
echo "full log: $LOG"
