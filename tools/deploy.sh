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
    # A DEAD HOLDER MUST NOT BLOCK FOREVER. A deploy that was killed, or one whose shell exited
    # without running its trap, left this file behind and every later deploy refused -- which is how
    # a revert of a CRASHING build silently did not happen, leaving the crash installed while the
    # log said the revert had been built. Staleness is decided by asking the OS whether the
    # recorded pid is still alive, not by a timeout.
    held=$(cat "$LOCK" 2>/dev/null)
    holder=${held%% *}
    if [ -n "$holder" ] && kill -0 "$holder" 2>/dev/null; then
        echo "another deploy is genuinely running (lock held by $held)"
        exit 1
    fi
    echo "clearing stale lock from dead pid $holder ($held)"
    rm -f "$LOCK"
    (set -o noclobber; echo "$$ $(date +%H:%M:%S)" > "$LOCK") 2>/dev/null || { echo "could not take lock"; exit 1; }
fi
trap 'rm -f "$LOCK"' EXIT

VERIFY_ONLY=0
[ "${1:-}" = "--verify-only" ] && VERIFY_ONLY=1
fail=0
warn=0

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

# UPLOAD ONLY WHAT CHANGED, because a full run over sys-ftpd takes about ten minutes and most of
# those 68 files are byte-identical to the last deploy. Ten minutes per iteration throttles
# everything upstream of it.
#
# SAFE BECAUSE SKIPPING IS NOT TRUSTED. The verification pass below still checks EVERY file's size
# against the card, and the repair pass re-uploads anything that disagrees -- so a wrong skip
# costs one extra round trip, not a broken install. The manifest is written only after
# verification passes, so a failed deploy cannot poison the next one into skipping.
#
# Pass --all to force a full upload.
MANIFEST="$ROOT/work/.deploy-manifest"
FORCE_ALL=0
[ "${1:-}" = "--all" ] && FORCE_ALL=1
declare -A PREV=()
if [ "$FORCE_ALL" = "0" ] && [ -f "$MANIFEST" ]; then
    # Two fields: "<size>:<mtime>\t<path>". The signature is stored as one opaque string so the
    # reader and the writer cannot disagree about how many columns there are -- which they briefly
    # did, and the effect was a manifest that silently loaded nothing and skipped no files.
    while IFS=$'\t' read -r msig mpath; do
        [ -n "${mpath:-}" ] && PREV["$mpath"]="$msig"
    done < "$MANIFEST"
fi

skipped=0
# SIGNATURES ARE SNAPSHOTTED AT UPLOAD TIME, not at verification time.
#
# The manifest used to be built from the files as they were when verification finished, which is
# minutes after the upload. Rebuilding while a deploy was in flight therefore recorded the NEW
# signature for a file whose OLD bytes were on the card, and the next deploy skipped it. Caught
# exactly that way: motion_list.bin verified as 45284 remote against 45412 local right after a
# rebuild landed mid-deploy. The verifier found it, which is what it is for -- but the manifest
# must not be able to lie in the first place.
declare -A SENT=()
for f in "${FILES[@]}"; do
    sig="$(stat -c%s "$f"):$(stat -c%Y "$f")"
    SENT["$f"]="$sig"
    if [ "${PREV[$f]:-}" = "$sig" ]; then
        skipped=$((skipped+1))
        continue
    fi
    enc=$(printf '%s' "$f" | sed 's/ /%20/g')
    printf "  %-56s " "$f"
    code=$(curl -s "${CURL_AUTH[@]}" --max-time 300 --ftp-create-dirs -T "$f" "$DEST/$enc" -w "%{http_code}")
    # A bad code is a WARNING, not a failure. VERIFICATION IS THE AUTHORITY -- which is the whole
    # lesson in this file's header, and this loop was contradicting it: a file that returned FTP 200
    # and then landed byte-perfect (or landed at zero bytes and was repaired) still forced
    # "DEPLOY HAD FAILURES", which trains you to ignore the word. Both directions of that mistake
    # have now been observed on real deploys. The only thing that decides the outcome is whether the
    # bytes on the card match the bytes here.
    if [ "$code" = "226" ]; then echo "ok"; else echo "warn ($code)"; warn=$((warn+1)); fi
done
[ "$skipped" -eq 0 ] || echo "  skipped $skipped unchanged file(s) -- verification below still checks every one"
[ "$warn" -eq 0 ] || echo "  $warn upload(s) returned an odd code -- verification below is what counts"

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
# Let sys-ftpd settle first. Verifying the instant a sixty-file upload finishes is measuring a
# saturated server, and both its LIST and its SIZE answers go unreliable there -- which is how one
# run produced 64 false "absent" reports.
sleep 3
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

    # A DIRECTORY LISTING IS NOT AUTHORITATIVE, and trusting it produced 64 false alarms in one
    # run: files uploaded minutes earlier reported "absent" and the repair pass then re-uploaded
    # all of them and still reported failure. sys-ftpd truncates LIST responses when it is busy --
    # which is exactly whenever Smash is running, i.e. whenever we most want to deploy.
    #
    # So a file that looks absent or wrong gets a targeted SIZE query before being believed. FTP's
    # SIZE is a single authoritative answer about one file, and it is only spent on the suspicious
    # ones, so the cached-listing optimisation (one request per directory instead of per file)
    # still holds for everything that looks fine.
    if [ "$got" != "$want" ]; then
        enc=$(printf '%s' "$f" | sed 's/ /%20/g')
        confirmed=$(curl -sI "${CURL_AUTH[@]}" --max-time 45 "$DEST/$enc" 2>/dev/null \
                    | tr -d '\r' | awk '/^Content-Length:/ { print $2; exit }')
        if [ -n "$confirmed" ] && [ "$confirmed" = "$want" ]; then
            # The listing lied. Nothing to do.
            got="$want"
        else
            got="${confirmed:-${got:-absent}}"
        fi
    fi

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
            # BACK OFF BETWEEN ATTEMPTS. The cause of a failed upload here is a saturated
            # sys-ftpd -- it has just taken sixty files while the game is running -- and retrying
            # immediately three times hammers the thing that is already struggling. Observed
            # directly: two files failed all three attempts, and both succeeded on a single manual
            # retry a moment later. One of them had in fact landed correctly and the SIZE query
            # itself returned "absent", so under load nothing it says is reliable.
            [ "$attempt" -gt 1 ] && sleep $(( (attempt - 1) * 4 ))
            printf "  %-52s try %d " "$f" "$attempt"
            code=$(curl -s "${CURL_AUTH[@]}" --max-time 300 --ftp-create-dirs -T "$f" "$DEST/$enc" -w "%{http_code}")
            # Confirm with a targeted SIZE, not the upload's return code and not a directory
            # listing. The return code lied once (FTP 200 on a file that landed at zero bytes) and
            # the listing lies whenever sys-ftpd is busy -- which is how the previous version of
            # this loop burned three attempts each on 36 files that were already correct.
            got=$(curl -sI "${CURL_AUTH[@]}" --max-time 45 "$DEST/$enc" 2>/dev/null \
                  | tr -d '\r' | awk '/^Content-Length:/ { print $2; exit }')
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
    # Only now is it safe to record what the card holds. Written after verification so a failed
    # deploy can never teach the next run to skip a file that never landed.
    if [ "$VERIFY_ONLY" = "0" ]; then
        : > "$MANIFEST"
        for f in "${FILES[@]}"; do
            # SENT[] is what was on disk when this file was uploaded or deliberately skipped, not
            # what is on disk now. A rebuild during the deploy must not be recorded as shipped.
            printf '%s	%s
' "${SENT[$f]:-0:0}" "$f" >> "$MANIFEST"
        done
    fi
else
    echo "  $bad file(s) wrong or missing on the card"
    fail=1
fi

echo
[ $fail -eq 0 ] && echo "DEPLOY OK" || { echo "DEPLOY HAD FAILURES"; exit 1; }
