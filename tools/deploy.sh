#!/usr/bin/env bash
# Runs the parity harness, then deploys the plugin and the whole mod folder to the console.
# Refuses to deploy if any check fails.
#
# FTP is only reachable when Smash is NOT running, until sys-ftpd takes effect on next boot.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOST=<console-ip>
PORT=5000
B="ftp://$HOST:$PORT"
NRO="$ROOT/pikachu-moveset/plugin/target/aarch64-skyline-switch/release/librat_of_doom.nro"
MOD="$ROOT/pikachu-moveset/mod"
DEST="$B/ultimate/mods/Rat%20of%20Doom"
PLUGINS="$B/atmosphere/contents/01006A800016E000/romfs/skyline/plugins"

echo "== parity harness =="
bash "$ROOT/tools/check.sh" || { echo; echo "REFUSING TO DEPLOY - checks failed"; exit 1; }

echo
if ! curl -s --connect-timeout 5 --max-time 10 -l "$B/" >/dev/null 2>&1; then
    echo "FTP unreachable on $HOST:$PORT."
    echo "Smash is probably running - ftpd-classic exits when a game launches."
    exit 1
fi

echo "== deploying plugin =="
printf "  %-56s " "librat_of_doom.nro"
curl -s --max-time 180 -T "$NRO" "$PLUGINS/librat_of_doom.nro" -w "http=%{http_code} %{size_upload}\n"

echo "== deploying mod folder =="
cd "$MOD"
fail=0
while read -r f; do
    enc=$(printf '%s' "$f" | sed 's/ /%20/g')
    printf "  %-56s " "$f"
    code=$(curl -s --max-time 300 --ftp-create-dirs -T "$f" "$DEST/$enc" -w "%{http_code}")
    if [ "$code" = "226" ]; then echo "ok"; else echo "FAILED ($code)"; fail=1; fi
done < <(find . -type f | sed 's|^\./||' | sort)

echo
[ $fail -eq 0 ] && echo "DEPLOY OK" || { echo "DEPLOY HAD FAILURES"; exit 1; }
