#!/usr/bin/env bash
# Files the CARD has inside the mod folder that the local mod tree does not ship.
#
# WHY THIS IS NOT THE RETIRE PASS. deploy.sh's retire pass works from the manifest -- what this
# machine believes it last put there -- so it can only retire what it knows it uploaded. A file
# that arrived some other way (an older deploy from a different checkout, a hand copy, an SD card
# cloned from another console) is invisible to it. ARCropolis does not care how a file arrived:
# if its path matches an arc path it is loaded. So an orphan is a live replacement nobody declared.
#
# THIS IS THE SHAPE OF THE 2026-10-17 FAULT, generalised. That one was stale CONTENT at paths we do
# ship; this finds stale PATHS we no longer ship at all. Both are "the card is not the tree".
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
. "$ROOT/tools/console.sh"
HOST=$CONSOLE_HOST
PORT=${FTP_PORT:-5000}
PASS="$(tr -d '\r\n' < "$ROOT/.ftp-credentials.local" 2>/dev/null)"
B="ftp://$HOST:$PORT"
REMOTE="/ultimate/mods/Rat of Doom"
# THE SHIPPED TREE, NOT THE AUTHORING ONE. `mod/` is written in the host's c00 coordinate
# system because that is what the generators and gates read; `ship/` is the same files moved
# onto Rat of Doom's added slot, and it is what the card must mirror exactly.
MOD="$ROOT/pikachu-moveset/ship"

enc() { printf '%s' "$1" | sed 's/ /%20/g'; }
listing() {
    local rel="$1"
    curl -s -u "poaga:$PASS" --max-time 40 "$B$(enc "$REMOTE")$([ -n "$rel" ] && enc "/$rel")/" 2>/dev/null | tr -d '\r'
}
walk() {
    local rel="$1" depth="$2"
    [ "$depth" -gt 8 ] && return
    listing "$rel" | while read -r perm l u g size m d t name; do
        [ -z "${name:-}" ] && continue
        local child="${rel:+$rel/}$name"
        if [ "${perm:0:1}" = "d" ]; then walk "$child" $((depth+1)); else echo "$child"; fi
    done
}

echo "  scanning $HOST:$PORT$REMOTE ..."
CARD=$(mktemp); LOCAL=$(mktemp); trap 'rm -f "$CARD" "$LOCAL"' EXIT
walk "" 1 | sort > "$CARD"
(cd "$MOD" && find . -type f | sed 's|^\./||') | sort > "$LOCAL"

n_card=$(wc -l < "$CARD"); n_local=$(wc -l < "$LOCAL")
echo "  card ships $n_card file(s); the tree ships $n_local"

orphans=$(comm -23 "$CARD" "$LOCAL")
missing=$(comm -13 "$CARD" "$LOCAL")
rc=0
if [ -n "$orphans" ]; then
    echo "  FAIL the card has file(s) the tree does not ship -- ARCropolis loads these anyway:"
    printf '%s\n' "$orphans" | sed 's/^/         /'
    rc=1
else
    echo "  ok  no orphans: every file on the card is one the tree ships"
fi
if [ -n "$missing" ]; then
    echo "  FAIL the tree ships file(s) the card does not have -- deploy did not complete:"
    printf '%s\n' "$missing" | head -20 | sed 's/^/         /'
    rc=1
else
    echo "  ok  every file the tree ships is present on the card"
fi
exit $rc
