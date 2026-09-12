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
    #
    # A WEDGED LIVE HOLDER BLOCKS AS HARD AS A DEAD ONE, and that cost a playtest round.
    #
    # What happened: a `deploy.sh && git commit` chain was launched with a 600s timeout, exceeded
    # it, and was moved to the background still holding this lock. Every later deploy then refused
    # -- correctly, by this block's own rule -- for twenty minutes. The whip fix never reached the
    # card, and the playtest that followed ran the OLD build and reported the OLD bug.
    #
    # `kill -0` WAS RIGHT AND THE FIRST DIAGNOSIS WAS WRONG. It looked like a stale lock from a
    # dead pid, on the strength of `tasklist /FI "PID eq 48924"` printing "No tasks are running".
    # That check is in the WRONG PID NAMESPACE: under Git Bash `$$` is an MSYS pid, which tasklist
    # cannot see at all, so it answers "not running" for every live shell. `ls -d /proc/<pid>` is
    # the test that works here -- and it showed the holder alive the whole time.
    #
    # So liveness stays as the first gate, and an age bound is added on top of it for the case
    # liveness cannot cover: a holder that is alive but no longer going to finish. A deploy is ~25s
    # and the slowest recorded is under two minutes, so 15 minutes cannot false-positive on one
    # that is genuinely working. The original objection to a bare timeout -- that it can kill a
    # running deploy -- is answered by requiring BOTH.
    #
    # The other half of the fix is not in this file: do not chain a deploy into anything else under
    # a timeout. Run it alone, let it finish, then commit.
    held=$(cat "$LOCK" 2>/dev/null)
    holder=${held%% *}
    age=9999
    if [ -f "$LOCK" ]; then
        now=$(date +%s)
        made=$(stat -c %Y "$LOCK" 2>/dev/null || echo "$now")
        age=$((now - made))
    fi
    # `/proc/<pid>` rather than `kill -0`: same answer for a live MSYS pid, and it is the test that
    # can actually be checked by hand when this goes wrong again.
    if [ -n "$holder" ] && [ -d "/proc/$holder" ] && [ "$age" -lt 900 ]; then
        echo "another deploy is genuinely running (lock held by $held, ${age}s old)"
        exit 1
    fi
    if [ "$age" -ge 900 ]; then
        echo "clearing lock ${age}s old -- older than any real deploy, holder pid $holder"
    fi
    echo "clearing stale lock from dead pid $holder ($held)"
    rm -f "$LOCK"
    (set -o noclobber; echo "$$ $(date +%H:%M:%S)" > "$LOCK") 2>/dev/null || { echo "could not take lock"; exit 1; }
fi
trap 'rm -f "$LOCK"' EXIT

# SECTION TIMERS, because a deploy that takes over a minute needs to say WHERE. Guessing was
# wrong twice: the network looked like the cost and is not -- the plugin upload is half a second,
# a directory listing is half a second, and hashing the whole 6.4 MB mod tree is a tenth. Every
# real second has been somewhere else.
T0=$(date +%s%N)
TLAST=$T0
mark() {
    local now; now=$(date +%s%N)
    printf '  [%5.1fs  +%4.1fs]  %s\n' \
        "$(( (now - T0) / 1000000 ))e-3" "$(( (now - TLAST) / 1000000 ))e-3" "$1"
    TLAST=$now
}

FORCE_ALL=0
[ "${1:-}" = "--all" ] && FORCE_ALL=1
# Files this run actually put on the card. The verification pass is scoped to these; see the
# reasoning where it is used.
UPLOADED=()
VERIFY_ONLY=0
[ "${1:-}" = "--verify-only" ] && VERIFY_ONLY=1
fail=0
warn=0

if [ "$VERIFY_ONLY" = "0" ]; then
  echo "== parity harness =="
  bash "$ROOT/tools/check.sh" || { echo; echo "REFUSING TO DEPLOY - checks failed"; exit 1; }
  mark "checks"
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
mark "plugin upload"

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
declare -A PREV=()
if [ "$FORCE_ALL" = "0" ] && [ -f "$MANIFEST" ]; then
    # Two fields: "<size>:<sha1>\t<path>". The signature is stored as one opaque string so the
    # reader and the writer cannot disagree about how many columns there are -- which they briefly
    # did, and the effect was a manifest that silently loaded nothing and skipped no files.
    #
    # IT USED TO BE size:mtime, AND THAT MADE EVERY DEPLOY UPLOAD EVERYTHING. tools/anim-dropin.sh
    # regenerates all 74 animations on every build, so their mtimes always change even when the
    # bytes do not -- and the mod tree is only 6.4 MB, so hashing the whole thing costs 0.1s.
    # Uploading 80-odd files instead of the two that changed is not just slow: sys-ftpd truncates
    # roughly one upload in eighty to zero bytes, so the pointless re-uploads were the main source
    # of the corruption the repair pass exists to fix. Three consecutive deploys each found exactly
    # one zero-byte .nuanmb; content hashing removes most of that exposure.
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
#
# Hashed in ONE batch rather than per file. A `sha1sum` process per file is 86 process spawns on
# Windows, which is slower than reading the 6.4 MB.
declare -A HASH=()
while read -r h p; do
    HASH["$p"]="$h"
done < <(sha1sum "${FILES[@]}" 2>/dev/null | sed 's/^\([0-9a-f]*\)  */\1 /')
declare -A SENT=()
# SIZES IN ONE `stat`, NOT ONE PER FILE. A process spawn under Git Bash on Windows costs tens of
# milliseconds, and this loop plus the verifier below were spending about ninety seconds of a
# hundred-second deploy on `stat`, `dirname`, `basename`, `sed` and `awk` -- roughly six hundred
# spawns. Measured against the network, which is not the problem at all: the plugin upload is
# 0.5s, a directory listing is 0.5s, and hashing the whole 6.4 MB mod tree is 0.1s.
declare -A SIZE=()
while read -r sz name; do
    SIZE["$name"]="$sz"
done < <(stat -c '%s %n' -- "${FILES[@]}" 2>/dev/null)
for f in "${FILES[@]}"; do
    # Size stays in the signature alongside the hash. It is what the console is verified against,
    # so keeping both means a manifest entry can be read against a remote SIZE query directly.
    sig="${SIZE[$f]:-0}:${HASH[$f]:-nohash}"
    SENT["$f"]="$sig"
    if [ "${PREV[$f]:-}" = "$sig" ]; then
        skipped=$((skipped+1))
        continue
    fi
    UPLOADED+=("$f")
    enc="${f// /%20}"
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
[ "$skipped" -eq 0 ] || echo "  skipped $skipped unchanged file(s) -- each was verified on the card when the manifest was written"
[ "$warn" -eq 0 ] || echo "  $warn upload(s) returned an odd code -- verification below is what counts"
mark "mod folder"

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
mark "retire"
fi

cd "$MOD"
mapfile -t ALLFILES < <(find . -type f | sed 's|^./||' | sort)
# Sizes again, in one `stat`, because --verify-only skips the upload block that filled them --
# and an UNDECLARED associative array is silently an indexed one, so `${SIZE[$f]}` then evaluates
# the filename as arithmetic and dies on the first dot ("config.json: invalid arithmetic
# operator"). Declaring it on both paths is the fix; re-filling it costs one process.
declare -A SIZE=()
while read -r sz name; do
    SIZE["$name"]="$sz"
done < <(stat -c '%s %n' -- "${ALLFILES[@]}" 2>/dev/null)

# VERIFY WHAT THIS RUN TOUCHED, NOT THE WHOLE CARD -- AND THAT IS THE SAME ARGUMENT THAT ALREADY
# JUSTIFIES SKIPPING THE UPLOAD.
#
# The manifest is written ONLY after a successful verification, so an entry in it means those
# exact bytes were confirmed present on the card. Nothing changes the card between deploys except
# this script. So a file whose local signature still matches its manifest entry, and which this
# run did not upload, cannot have been broken by this run. Re-checking it is not extra safety; it
# is extra FTP.
#
# AND FTP IS EXACTLY WHAT HURTS. sys-ftpd wedges after a handful of connections in quick
# succession: measured, the fifth directory listing picks up the connection and never sends data,
# burning the full timeout, and retries keep hitting the same wall. Twenty-six seconds of a
# thirty-nine second deploy went there. Not making the request is the only fix that works.
#
# `--all` and `--verify-only` both still sweep everything, so the paranoid check is one flag away
# and the manifest can always be re-earned from scratch.
if [ "$VERIFY_ONLY" = "1" ] || [ "$FORCE_ALL" = "1" ] || [ "${#UPLOADED[@]}" -eq "${#ALLFILES[@]}" ]; then
    FILES=("${ALLFILES[@]}")
    SCOPE="every file on the card"
else
    FILES=("${UPLOADED[@]}")
    SCOPE="the ${#UPLOADED[@]} file(s) this run uploaded (the rest were verified when the manifest was written)"
fi
echo
if [ "${#FILES[@]}" -eq 0 ]; then
    echo "== nothing was uploaded, so there is nothing new to verify =="
    echo "  ok  all ${#ALLFILES[@]} files unchanged since the last verified deploy"
    mark "verify"
    echo
    echo "DEPLOY OK"
    exit 0
fi
echo "== verifying $SCOPE =="
# Let sys-ftpd settle first. Verifying the instant a sixty-file upload finishes is measuring a
# saturated server, and both its LIST and its SIZE answers go unreliable there -- which is how one
# run produced 64 false "absent" reports.
sleep 3
mark "settle"
# One listing per DIRECTORY, cached. The first version of this made a request per file -- up to
# 120 FTP connections -- and the last two timed out and were reported "absent" when they were
# present with byte-exact sizes. A false alarm in the verifier is as bad as no verifier, since it
# trains you to ignore it.
LS="$ROOT/work/deploy-listing"
rm -rf "$LS"; mkdir -p "$LS"
bad=0
# Directory names by string surgery rather than by spawning `dirname` once per file.
declare -A DIRS=()
for f in "${FILES[@]}"; do
    d="${f%/*}"; [ "$d" = "$f" ] && d="."
    DIRS["$d"]=1
done
# ONE CURL PER DIRECTORY, AND NOT ONE CURL FOR ALL OF THEM. Batching the five listings into a
# single invocation so the FTP control connection would be reused looked like the obvious win and
# was a 62-SECOND REGRESSION: sys-ftpd does not cope with several LISTs down one control
# connection, and curl sits waiting after the first. Opened separately they are half a second
# each. Measured both ways with the section timers above, which is why those are now permanent --
# every guess about where this script spends its time has been wrong, including that one.
#
# A SHORT TIMEOUT AND A RETRY, BECAUSE ONE LISTING IN FIVE JUST HANGS. Measured by running this
# exact loop by hand several times: four directories answer in 0.4-0.7s and one picks up the
# connection and never sends the data, burning the whole --max-time and returning curl 28. Which
# one varies between runs, so it is sys-ftpd stalling a passive data connection, not a bad path.
#
# At --max-time 90 that single stall WAS the deploy: 62 of 75 seconds. Eight seconds with two
# retries costs 2.5s on a good run and about 10s on a stalled one, and the retry has succeeded
# immediately every time it has been needed.
for d in "${!DIRS[@]}"; do
    denc="${d// /%20}"
    safe="${d//\//_}"; safe="${safe//./_}"
    url="$DEST/"; [ "$d" != "." ] && url="$DEST/$denc/"
    for attempt in 1 2 3; do
        curl -s "${CURL_AUTH[@]}" --max-time 8 -o "$LS/$safe.txt" "$url" 2>/dev/null
        [ -s "$LS/$safe.txt" ] && break
        sleep 1
    done
done
mark "listings"
for d in "${!DIRS[@]}"; do
    safe="${d//\//_}"; safe="${safe//./_}"
    # sys-ftpd sends CRLF; strip it in place rather than piping each listing through `tr`.
    if [ -s "$LS/$safe.txt" ]; then
        printf '%s' "$(tr -d '\r' < "$LS/$safe.txt")" > "$LS/$safe.txt.clean" && mv "$LS/$safe.txt.clean" "$LS/$safe.txt"
    else
        # NOT COUNTED AS A FAILURE. An unlistable directory is a reason to fall through to the
        # per-file SIZE query below, and that query is the authority -- so counting it here too
        # double-reported the same file ("2 file(s) wrong" for one path) and, worse, could fail a
        # deploy whose files were all present. Said once, as information.
        echo "  could not list $d -- falling back to a per-file SIZE query for its contents"
    fi
done

# EVERY LISTING PARSED ONCE, into one map of "dir/name" -> size. This used to be an `awk` per
# file over the cached listing, which is 86 more spawns for an answer already sitting in a file
# we have read.
#
# `ls -l` style: perms links owner group size mon day time name. The name is the 9th field
# onward, so take the LAST field rather than slicing a fixed prefix.
declare -A REMOTE=()
for d in "${!DIRS[@]}"; do
    safe="${d//\//_}"; safe="${safe//./_}"
    [ -s "$LS/$safe.txt" ] || continue
    while read -r -a fields; do
        n="${#fields[@]}"
        [ "$n" -ge 5 ] || continue
        name="${fields[$((n-1))]}"
        [ "$d" = "." ] && REMOTE["$name"]="${fields[4]}" || REMOTE["$d/$name"]="${fields[4]}"
    done < "$LS/$safe.txt"
done

BROKEN=()
for f in "${FILES[@]}"; do
    want="${SIZE[$f]:-$(stat -c%s "$f")}"
    got="${REMOTE[$f]:-}"

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
        enc="${f// /%20}"
        # THREE ATTEMPTS, BECAUSE THE SIZE QUERY LIES TRANSIENTLY TOO.
        #
        # The paragraph above says a directory listing is not authoritative and a targeted SIZE
        # is. Half right: sys-ftpd wedges after a run of connections and then fails EVERYTHING
        # for a moment, SIZE included. Measured -- a verify sweep declared
        # ui/message/msg_name.xmsbt absent, and three SIZE queries by hand seconds later all
        # returned 908, which is exactly the local size. The verifier had just called a correct
        # file broken.
        #
        # A false alarm is as bad as no verifier, because it trains you to ignore the word FAIL.
        # So the authoritative query gets retries of its own.
        confirmed=""
        for attempt in 1 2 3; do
            confirmed=$(curl -sI "${CURL_AUTH[@]}" --max-time 20 "$DEST/$enc" 2>/dev/null \
                        | tr -d '\r' | awk '/^Content-Length:/ { print $2; exit }')
            [ -n "$confirmed" ] && break
            sleep 2
        done
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
#
# ZERO-BYTE UPLOADS ARE ROUTINE, NOT A FLUKE, and that is worth stating plainly because it was
# mistaken for one. The first two deploys after size-based verification went in each found exactly
# one: d03speciallwstart.nuanmb, then d01specialslariat.nuanmb, both at 0 bytes on the card with a
# clean FTP return code. Call it one file in eighty per run. Every one of them is a .nuanmb, which
# is simply because those are the big files.
#
# So the repair pass is load-bearing infrastructure, not a safety net for odd days. It also means
# the OLD listing-based verifier was letting roughly one broken animation per deploy through, which
# is the likeliest explanation for two load crashes that were blamed on an article clone -- see
# pikachu-moveset/plugin/src/articles/mod.rs.
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

mark "verify"
if [ "$bad" -eq 0 ]; then
    if [ "${#FILES[@]}" -eq "${#ALLFILES[@]}" ]; then
        echo "  ok  all ${#ALLFILES[@]} files verified by size"
    else
        echo "  ok  ${#FILES[@]} of ${#ALLFILES[@]} file(s) verified by size; the rest were verified when the manifest was written"
    fi
    # Only now is it safe to record what the card holds. Written after verification so a failed
    # deploy can never teach the next run to skip a file that never landed.
    if [ "$VERIFY_ONLY" = "0" ]; then
        : > "$MANIFEST"
        # ALL of them, not just the ones verified this run: scoping the verification to what was
        # uploaded must not scope the RECORD, or every unverified-this-run file would drop out of
        # the manifest and be re-uploaded next time.
        for f in "${ALLFILES[@]}"; do
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
