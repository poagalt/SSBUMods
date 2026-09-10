#!/usr/bin/env bash
# Runs the parity harness, deploys the plugin and the whole mod folder to the console, then
# VERIFIES every uploaded file against its local size. Refuses to deploy if any check fails.
#
# FTP works REGARDLESS of what the console is running, as long as the login is supplied -- see the
# credentials block below. It used to require Smash closed and ftpd-classic launched by hand.
#
# TWO LESSONS ARE BUILT IN HERE, both paid for:
#
# 1. A LOCK. Two queued deploys once fired 2 seconds apart, raced each other over the same FTP
#    server, and both were cut off after 48 of 60 files. The console ended up with a partial mod,
#    and three playtest reports ("charging animations still pikachu's", "landing animations still
#    pikachu's", "dtilt/dsmash still pikachu's") were all just files that never arrived.
#
# 2. VERIFICATION. Uploading and reporting "ok" per file was not enough -- a truncated run still
#    printed ok for everything it managed. Nothing catches a partial deploy except comparing what
#    is on the card against what should be. Now every file's remote size is checked, and the
#    deploy fails loudly on any mismatch or omission.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOST=<console-ip>
PORT=5000
B="ftp://$HOST:$PORT"

# CREDENTIALS, AND WHY THIS MATTERS MORE THAN IT LOOKS.
#
# There are TWO FTP servers in play on this console and they behave differently:
#
#   ftpd-classic   hand-launched from hbmenu, ANONYMOUS, and it exits the moment a game starts.
#   sys-ftpd       a background SYSMODULE at atmosphere/contents/420000000000000E with a
#                  boot2.flag, so it runs from boot and does not care what else is running --
#                  but config/sys-ftpd/config.ini sets anonymous:=0, so it REQUIRES a login.
#
# Every FTP call here used to be credential-less, which silently meant "only works against
# ftpd-classic", which in turn meant every deploy needed Smash closed AND ftpd launched by hand.
# A credential-less request to sys-ftpd returns FTP 430 (invalid username or password) -- and
# because 430 is not an obviously-auth-shaped failure, it read for a long time as "FTP is down".
#
# With the login supplied, sys-ftpd answers regardless of what the console is doing, which is what
# makes deploying and reading crash reports possible without asking for anything.
FTP_USER=poaga
FTP_PASS="$(cat "$ROOT/.ftp-credentials.local" 2>/dev/null | tr -d '\r\n')"
CURL_AUTH=()
if [ -n "$FTP_PASS" ]; then
    CURL_AUTH=(-u "$FTP_USER:$FTP_PASS")
fi
NRO="$ROOT/pikachu-moveset/plugin/target/aarch64-skyline-switch/release/librat_of_doom.nro"
MOD="$ROOT/pikachu-moveset/mod"
DEST="$B/ultimate/mods/Rat%20of%20Doom"
REMOTE_ROOT="/ultimate/mods/Rat of Doom"
PLUGINS="$B/atmosphere/contents/01006A800016E000/romfs/skyline/plugins"
LOCK="$ROOT/work/.deploy.lock"
mkdir -p "$ROOT/work"

# Single-instance lock. `set -o noclobber` makes the redirect fail if the file already exists,
# which is atomic enough for one machine talking to one console.
if ! (set -o noclobber; echo "$$ $(date +%H:%M:%S)" > "$LOCK") 2>/dev/null; then
    echo "another deploy is already running (lock held by $(cat "$LOCK" 2>/dev/null))"
    echo "if that is stale: rm $LOCK"
    exit 1
fi
trap 'rm -f "$LOCK"' EXIT

VERIFY_ONLY=0
[ "${1:-}" = "--verify-only" ] && VERIFY_ONLY=1
fail=0

if [ "$VERIFY_ONLY" = "0" ]; then
  echo "== parity harness =="
  bash "$ROOT/tools/check.sh" || { echo; echo "REFUSING TO DEPLOY - checks failed"; exit 1; }
fi

echo
if ! curl -s "${CURL_AUTH[@]}" --connect-timeout 5 --max-time 10 -l "$B/" >/dev/null 2>&1; then
    echo "FTP unreachable on $HOST:$PORT (sys-ftpd should answer regardless of what is running)."
    exit 1
fi

if [ "$VERIFY_ONLY" = "1" ]; then
  echo "== verify only: not uploading anything =="
else
echo "== deploying plugin =="
printf "  %-56s " "librat_of_doom.nro"
curl -s "${CURL_AUTH[@]}" --max-time 180 -T "$NRO" "$PLUGINS/librat_of_doom.nro" -w "http=%{http_code} %{size_upload}\n"

echo "== deploying mod folder =="
cd "$MOD"
mapfile -t FILES < <(find . -type f | sed 's|^\./||' | sort)
for f in "${FILES[@]}"; do
    enc=$(printf '%s' "$f" | sed 's/ /%20/g')
    printf "  %-56s " "$f"
    code=$(curl -s "${CURL_AUTH[@]}" --max-time 300 --ftp-create-dirs -T "$f" "$DEST/$enc" -w "%{http_code}")
    if [ "$code" = "226" ]; then echo "ok"; else echo "FAILED ($code)"; fail=1; fi
done

# Files the mod used to ship and must no longer be present. Uploading cannot remove anything, so
# a file dropped from the mod folder would otherwise linger and keep overriding vanilla --
# `vl.prc` declared a 21st article param with no weapon behind it. Explicit rather than a diff of
# the remote tree, so this can never delete something unrelated.
# vl.prc is back here because the param_special_s override is disabled -- it null-dereferenced in
# smashline load_fighter_effects on fighter load. See tools/gen-vlprc.sh. The loop SKIPs anything
# still present in the mod tree, so this stays correct if the override is ever re-enabled.
RETIRED="
fighter/pikachu/param/vl.prc
"
echo
echo "== retiring files the mod no longer ships =="
while read -r f; do
    [ -z "${f// }" ] && continue
    if [ -f "$MOD/$f" ]; then
        echo "  SKIP  $f is still shipped -- remove it from RETIRED"; continue
    fi
    printf "  %-56s " "$f"
    out=$(curl -s "${CURL_AUTH[@]}" --max-time 60 -Q "-DELE $REMOTE_ROOT/$f" "$DEST/" 2>&1)
    case "$out" in
        *550*) echo "already gone" ;;
        *)     echo "deleted" ;;
    esac
done <<< "$RETIRED"
fi

cd "$MOD"
mapfile -t FILES < <(find . -type f | sed 's|^./||' | sort)
echo
echo "== verifying every file on the card matches what we shipped =="
# One listing per DIRECTORY, cached. The first version of this made a request per file -- up to
# 120 FTP connections -- and the last two timed out and were reported "absent" when they were
# present with byte-exact sizes. A false alarm in the verifier is as bad as no verifier, since it
# trains you to ignore it.
LS="$ROOT/work/deploy-listing"
rm -rf "$LS"; mkdir -p "$LS"
bad=0
for d in $(printf '%s\n' "${FILES[@]}" | xargs -n1 dirname | sort -u); do
    denc=$(printf '%s' "$d" | sed 's/ /%20/g')
    safe=$(printf '%s' "$d" | tr '/.' '__')
    url="$DEST/"; [ "$d" != "." ] && url="$DEST/$denc/"
    curl -s "${CURL_AUTH[@]}" --max-time 90 "$url" 2>/dev/null | tr -d '\r' > "$LS/$safe.txt"
    if [ ! -s "$LS/$safe.txt" ]; then
        echo "  could not list $d -- cannot verify it"; bad=$((bad+1))
    fi
done
BROKEN=()
for f in "${FILES[@]}"; do
    want=$(stat -c%s "$f")
    d=$(dirname "$f"); b=$(basename "$f")
    safe=$(printf '%s' "$d" | tr '/.' '__')
    # `ls -l` style: perms links owner group size mon day time name. The name is the 9th field
    # onward, so compare the last field rather than slicing a fixed prefix.
    got=$(awk -v n="$b" '$NF == n { print $5; exit }' "$LS/$safe.txt" 2>/dev/null)
    if [ "$got" != "$want" ]; then
        printf "  MISMATCH %-52s local %s remote %s\n" "$f" "$want" "${got:-absent}"
        bad=$((bad+1))
        BROKEN+=("$f")
    fi
done

# A REPAIR PASS, because sys-ftpd drops files under load and noticing by hand does not scale.
#
# Observed on a single 69-file deploy: one upload returned FTP 200 instead of 226, one landed at
# zero bytes, and one directory listing came back empty so its two files could not be verified at
# all. The verifier caught every one -- that is what it is for -- but a deploy that reports failure
# and stops is a deploy that needs a human, and the whole point of the credentials work was to stop
# needing one. So re-upload exactly the files that failed and check them again.
#
# Bounded and per-file: this retries the named mismatches only, never a blanket re-run, so a
# genuinely broken file cannot hide behind an endless loop.
if [ "$bad" -gt 0 ] && [ "$VERIFY_ONLY" = "0" ]; then
    echo
    echo "== repairing ${#BROKEN[@]} file(s) =="
    still=0
    for f in "${BROKEN[@]}"; do
        want=$(stat -c%s "$f")
        enc=$(printf '%s' "$f" | sed 's/ /%20/g')
        ok=0
        for attempt in 1 2 3; do
            printf "  %-52s try %d " "$f" "$attempt"
            code=$(curl -s "${CURL_AUTH[@]}" --max-time 300 --ftp-create-dirs -T "$f" "$DEST/$enc" -w "%{http_code}")
            # Re-list just this file's directory rather than trusting the upload's return code --
            # the zero-byte case reported success.
            denc=$(printf '%s' "$(dirname "$f")" | sed 's/ /%20/g')
            url="$DEST/"; [ "$(dirname "$f")" != "." ] && url="$DEST/$denc/"
            got=$(curl -s "${CURL_AUTH[@]}" --max-time 90 "$url" 2>/dev/null | tr -d '\r' \
                  | awk -v n="$(basename "$f")" '$NF == n { print $5; exit }')
            if [ "$got" = "$want" ]; then echo "ok ($code)"; ok=1; break; fi
            echo "still wrong (${got:-absent})"
        done
        [ "$ok" = "1" ] || still=$((still+1))
    done
    if [ "$still" -eq 0 ]; then
        echo "  ok  all ${#BROKEN[@]} repaired, all ${#FILES[@]} files now verified"
        bad=0
    else
        echo "  $still file(s) still wrong after 3 attempts"
    fi
fi

if [ "$bad" -eq 0 ]; then
    echo "  ok  all ${#FILES[@]} files verified by size"
else
    echo "  $bad file(s) wrong or missing on the card"
    fail=1
fi

echo
[ $fail -eq 0 ] && echo "DEPLOY OK" || { echo "DEPLOY HAD FAILURES"; exit 1; }
