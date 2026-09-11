#!/usr/bin/env bash
# Pulls the dump of the GAME'S OWN executable off the card: its symbol table, and its .text.
#
# WHY THIS IS THE UNBLOCK. Every one of these moves is a reimplementation rather than a
# drag-and-drop for exactly one reason: the part that decides "when this catch connects, put the
# victim there and me here" is compiled C++ in `main` and keys on the fighter kind. It is not in
# data.arc, it is not in the fighter's lua2cpp NRO, and `disasm --sym catch` over the whole gaogaen
# NRO proves the absence rather than merely failing to find it.
#
# A Skyline plugin runs INSIDE the game process, so `main` is mapped memory and its .dynsym is
# reachable the same way an NRO's is. src/telemetry/exedump.rs walks MOD0 -> .dynamic -> .dynsym
# and writes the names; that is what makes 58 MB of ARM navigable instead of hopeless.
#
#   1. ENABLED and DUMP_TEXT are both true in exedump.rs, so the build already dumps.
#   2. Load into training mode once with the modified Pikachu. It stalls for a few seconds.
#   3. bash tools/pull-exedump.sh
#
# The symbol file comes first and is small; .text is tens of megabytes and takes a few minutes
# over sys-ftpd. Both are pulled with retries, because sys-ftpd wedges after a run of connections
# (see tools/deploy.sh) and a truncated 58 MB transfer is worth noticing rather than parsing.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
OUT=vendor/main
mkdir -p "$OUT"

HOST=<console-ip>
PORT=5000
B="ftp://$HOST:$PORT"
FTP_USER=poaga
FTP_PASS="$(cat "$ROOT/.ftp-credentials.local" 2>/dev/null | tr -d '\r\n')"
A=()
[ -n "$FTP_PASS" ] && A=(-u "$FTP_USER:$FTP_PASS")

get() {
    local name min dest
    name="$1"; min="$2"; dest="$OUT/$name"
    local want
    want=$(curl -sI "${A[@]}" --max-time 30 "$B/$name" 2>/dev/null \
           | tr -d '\r' | awk '/^Content-Length:/ { print $2; exit }')
    if [ -z "$want" ]; then
        echo "  $name: not on the card"
        echo "     Is ENABLED true in src/telemetry/exedump.rs, and has a match been loaded since"
        echo "     the deploy? The dump happens on the first frame of a Pikachu object."
        return 1
    fi
    echo "  $name: card says $want bytes"
    for attempt in 1 2 3; do
        # --max-time scales with the file: 58 MB at the ~330 KB/s sys-ftpd manages is three
        # minutes, and a ceiling below that would truncate every time and look like corruption.
        curl -s "${A[@]}" --max-time 900 -o "$dest" "$B/$name"
        local got
        got=$(stat -c%s "$dest" 2>/dev/null || echo 0)
        if [ "$got" = "$want" ] && [ "$got" -ge "$min" ]; then
            echo "     pulled $got bytes, complete"
            return 0
        fi
        echo "     attempt $attempt got $got of $want -- retrying"
        sleep 3
    done
    echo "     FAILED after 3 attempts; keeping the partial file, it is still a usable prefix"
    return 1
}

echo "== the game's own symbol table and code =="
get rat-exedump.txt 16 || true
get rat-main-symbols.txt 100000 || true
get rat-main-text.bin 1000000 || true

echo
if [ -s "$OUT/rat-main-symbols.txt" ]; then
    echo "symbols: $(grep -vc '^#' "$OUT/rat-main-symbols.txt") entries"
    echo
    echo "the ones this project has been blocked on:"
    for pat in FighterGaogaen FighterDonkey FighterGamewatch Gaogaen Donkey; do
        n=$(grep -c "$pat" "$OUT/rat-main-symbols.txt" || true)
        printf '  %-18s %s\n' "$pat" "$n"
    done
fi
[ -s "$OUT/rat-exedump.txt" ] && { echo; echo "bases:"; sed 's/^/  /' "$OUT/rat-exedump.txt"; }
