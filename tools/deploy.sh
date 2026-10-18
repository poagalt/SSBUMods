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
# WHICH CARD: tools/console.sh, shared by every tool that talks to a console. There are two -- the
# 13.0.5 development card and the 13.0.4 one being played -- and the address lives in
# `.console-host.local` (FTP_HOST overrides it per command). No built-in default; see that file.
. "$ROOT/tools/console.sh"
HOST=$CONSOLE_HOST
PORT=${FTP_PORT:-5000}
B="ftp://$HOST:$PORT"
echo "  deploying to $HOST (tools/console.sh)"

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
# THE SHIPPED TREE, NOT THE AUTHORING ONE. `mod/` is written in the host's c00 coordinate
# system because that is what the generators and gates read; `ship/` is the same files moved
# onto Rat of Doom's added slot, and it is what the card must mirror exactly.
MOD="$ROOT/pikachu-moveset/ship"
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
# SKIPPING THE RETIRE PASS, for iterating. Each retirement is one DELE on its own connection at
# roughly a second, so dropping ARTICLE_SLOTS from `all` to `c00` leaves 626 costume-slot files to
# delete -- ten minutes of a deploy that otherwise moves 87 files. They are harmless while they
# sit there: gen-config.sh regenerates config.json from the tree, so an un-retired file is
# shipped-but-undeclared and never loads.
#
# IT IS NOT FREE, THOUGH, AND THAT IS WHY IT IS OPT-IN RATHER THAN THE DEFAULT. A card that holds
# files the tree does not is exactly the divergence that cost a playtest this week -- the plugin
# was a build behind and nothing said so. Run a plain `deploy.sh` before anything that matters.
SKIP_RETIRE=0
[ "${1:-}" = "--no-retire" ] && SKIP_RETIRE=1
[ "${SKIP_RETIRE:-0}" = "1" ] && SKIP_RETIRE=1
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

# A GUARD THAT REFUSED TO DEPLOY WHILE THE GAME RAN WAS HERE, AND IT WAS WRONG. Removed
# 2026-09-19, same day it was added.
#
# It was justified by ONE mid-match deploy that left three files absent after three retries each.
# The counter-evidence was in the very next log and went unread: the deploy immediately after it,
# with Smash CLOSED, needed FIVE repairs and could not list one directory at all. Failures happen
# in both states, which is why the verify-and-repair pass below exists and why this file's own
# header has always said "FTP works REGARDLESS of what the console is running, as long as the
# login is supplied". Deploying mid-match is how this project worked before the guard existed.
#
# The real protection is verification, not abstinence. One bad upload out of eighty is the
# measured rate; the repair pass catches it either way.

if [ "$VERIFY_ONLY" = "1" ]; then
  echo "== verify only: not uploading anything =="
else
echo "== deploying plugin =="
printf "  %-56s " "librat_of_doom.nro"
curl -s "${CURL_AUTH[@]}" --max-time 180 -T "$NRO" "$PLUGINS/librat_of_doom.nro" -w "http=%{http_code} %{size_upload}\n"
mark "plugin upload"

# READ THE PLUGIN BACK AND COMPARE IT BYTE FOR BYTE, EVERY RUN.
#
# This is the one check whose absence has actually cost playtests. The plugin is deployed outside
# the mod folder, so it was never in `UPLOADED` or `ALLFILES` -- which meant the verification pass
# below could not see it, and on any run where the mod tree happened to be unchanged the deploy
# printed "nothing was uploaded, so there is nothing new to verify" and exited 0 while saying
# nothing at all about the only file that had changed. That is exactly the state that produced two
# consecutive rounds of "behaviour is the same": the card was holding md5 367475d6, an older build
# whose own log said `CLONE_ENABLED=false`, while the local tree was 00e6ce31.
#
# sys-ftpd truncates roughly one upload in eighty to zero bytes -- measured, and the reason the mod
# folder gets a size check at all -- so an unverified 254 KB upload is not a theoretical risk. A
# read-back costs about a second and is the only thing that can tell "uploaded" from "landed".
#
# curl, not tools/ftpq.sh: ftpq strips NULs, which is harmless for a listing and fatal for an
# aarch64 binary.
printf "  %-56s " "verify plugin"
PLUGIN_BACK="$ROOT/work/.plugin-readback"
rm -f "$PLUGIN_BACK"
curl -s "${CURL_AUTH[@]}" --max-time 180 -o "$PLUGIN_BACK" "$PLUGINS/librat_of_doom.nro" || true
LOCAL_MD5=$(md5sum < "$NRO" | cut -d' ' -f1)
CARD_MD5=$(md5sum < "$PLUGIN_BACK" 2>/dev/null | cut -d' ' -f1)
if [ -s "$PLUGIN_BACK" ] && [ "$LOCAL_MD5" = "$CARD_MD5" ]; then
    echo "byte-identical on card, md5 ${LOCAL_MD5:0:8}"
    rm -f "$PLUGIN_BACK"
else
    echo "MISMATCH"
    echo "  !! the plugin on the card is not the plugin that was just built." >&2
    echo "     local $LOCAL_MD5 $(stat -c%s "$NRO") bytes" >&2
    echo "     card  ${CARD_MD5:-<absent>} $(stat -c%s "$PLUGIN_BACK" 2>/dev/null || echo 0) bytes" >&2
    echo "     re-run tools/deploy.sh; do NOT playtest this build." >&2
    exit 1
fi
mark "plugin verify"

echo "== deploying mod folder =="
cd "$MOD"
mapfile -t FILES < <(find . -type f | sed 's|^\./||' | sort)

# UPLOAD ONLY WHAT CHANGED, because a full run over sys-ftpd takes about ten minutes and most of
# those 68 files are byte-identical to the last deploy. Ten minutes per iteration throttles
# everything upstream of it.
#
# SKIPPING IS TRUSTED, AND THAT COMMENT USED TO SAY OTHERWISE. The verification pass only checks
# the files THIS run uploaded (see the VERIFY_ONLY/FORCE_ALL condition further down), so a
# wrong skip IS a broken install and nothing downstream catches it. The claim that every file
# was re-checked was false and it cost a whole deploy: on 2026-10-17 this script was pointed at
# a second console carrying a months-old mod tree, loaded the FIRST console's manifest, skipped
# 827 files as 'unchanged' and left Chrom's jab and Incineroar's back throw on the card -- which
# then crashed, because the plugin's ACMD expects the animation it shipped.
#
# Pass --all to force a full upload.
# ONE MANIFEST PER CONSOLE. It records what was verified ON A CARD, so it is only meaningful for
# the card it was written against -- and there are now two. A single shared file silently means
# "whatever the last card I talked to had", which is how the deploy above went wrong. Keying it
# by host makes a never-before-seen console start with no manifest, i.e. a full upload and a full
# verify, which is exactly the right default for one.
MANIFEST="$ROOT/work/.deploy-manifest-$HOST"
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
#
# THE `*` IS NOT DECORATION, AND MISSING IT MADE THIS WHOLE MECHANISM INERT.
#
# `sha1sum` on Git Bash reports in BINARY mode, so it prints `<hash> *<path>` -- the asterisk is
# the mode marker. The old `sed 's/^\([0-9a-f]*\)  */\1 /'` collapsed the double space and left
# the asterisk, so `read -r h p` got `p = "*fighter/..."`, every `HASH[$f]` lookup missed, and the
# signature fell through to its `nohash` default. All 639 manifest entries read `<size>:nohash`.
#
# WHAT THAT COST: with no hash, "has this file changed" was decided on SIZE ALONE, so any edit
# that does not change the byte count was never re-uploaded. That is not a corner case -- it is
# exactly what editing floats in a binary looks like. It hid the `numshexb` bounding-sphere fix:
# same 4592 bytes, different sphere centres, card left holding the old file while the local tree
# and every gate said the fix was in. Found only by pulling the bytes back and comparing md5.
#
# `[ *]\?` accepts either mode marker, so this is right in text mode too.
declare -A HASH=()
while read -r h p; do
    HASH["$p"]="$h"
done < <(sha1sum "${FILES[@]}" 2>/dev/null | sed 's/^\([0-9a-f]\{40\}\) [ *]\?/\1 /')
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
# `vl.prc` declared a 21st article param with no weapon behind it.
#
# TWO SOURCES, AND THE HAND-WRITTEN ONE ALONE WAS NOT ENOUGH.
#
# This used to be the RETIRED list below and nothing else, justified as "explicit rather than a
# diff of the remote tree, so this can never delete something unrelated". That reasoning is sound
# about safety and silent about coverage, and coverage is where it failed. Turning the article
# clone off removed 24 spikeball files from the mod tree, gen-config.sh correctly stopped
# declaring them, and all 24 stayed on the card -- shipped-but-undeclared, which is the exact
# state gen-config.sh's header calls unreachable. It had to be cleared by hand twice.
#
# That is the parallel-list failure mode gen-config.sh names as the cause of three earlier
# defects (throw_f, DONKEY, topscale's MAP), so it gets the same fix: compute it.
#
# THE MANIFEST IS THE RIGHT SOURCE, and it costs no FTP at all. It is rewritten only after a
# successful verification, so at THIS point in the run it still describes what is on the card --
# anything in it the mod tree no longer has is a file this script put there and no longer ships.
# A remote diff would need a recursive walk of directories the local tree does not know about,
# and the listing machinery below is already fighting sys-ftpd stalls for the ones it does.
#
# The loop SKIPs anything still present in the mod tree, so both sources stay correct if a file
# comes back -- which is why vl.prc can sit in RETIRED permanently.
RETIRED="
fighter/pikachu/param/vl.prc
"
# A FLOOR ON THE TREE SIZE, because this source is computed and the hand-written one was not.
# With a missing or half-built mod tree every manifest entry would read as retired and this would
# delete the install. 50 is well under the real 98 and well above any partial build.
STALE=""
n_local=$(find "$MOD" -type f 2>/dev/null | wc -l)
if [ "$n_local" -lt 50 ]; then
    echo
    echo "== NOT retiring anything: the mod tree has only $n_local file(s) =="
    echo "   Under 50 means a missing or half-built tree, where every manifest entry reads as"
    echo "   retired. Build first; nothing was deleted."
elif [ -f "$MANIFEST" ]; then
    while IFS=$'\t' read -r _msig mpath; do
        if [ -n "${mpath:-}" ] && [ ! -f "$MOD/$mpath" ]; then
            STALE="$STALE$mpath
"
        fi
    done < "$MANIFEST"
fi
echo
if [ "$SKIP_RETIRE" = "1" ]; then
    n_stale=$(printf '%s\n' "$RETIRED
$STALE" | grep -c . || true)
    echo "== retiring files the mod no longer ships -- SKIPPED (--no-retire), $n_stale pending =="
    echo "  they are undeclared in config.json and never load; run a plain deploy.sh to clear them"
    mark "retire"
else
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
done <<< "$RETIRED
$STALE"
mark "retire"
fi
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
    echo "== no MOD FILE was uploaded, so there is nothing new to verify there =="
    echo "  ok  all ${#ALLFILES[@]} mod files unchanged since the last verified deploy"
    echo "      (the plugin is read back and compared on every run, above, whatever this says)"
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
            # THE REPAIR UPLOAD DROPS `--ftp-create-dirs` FROM THE SECOND TRY ON, AS AN EXPERIMENT
            # WITH A KNOWN CONTROL.
            #
            # `fighter/pikachu/model/cshot/c00/model.xmb` came out of every deploy run at zero
            # bytes -- four runs, twelve attempts -- while the same file sent by hand with a bare
            # `curl -T`, no `--ftp-create-dirs`, landed at 1108 and byte-matched on RETR every
            # single time. That flag is the ONE difference between the command that fails and the
            # command that works, and by the repair stage the directory provably exists (its
            # fifteen siblings just verified). So try 1 is unchanged, and tries 2-3 are the
            # working command. The log line says which variant landed it, which settles the cause
            # on the next run with the console up instead of adding another theory.
            if [ "$attempt" -eq 1 ]; then
                variant="create-dirs"
                code=$(curl -s "${CURL_AUTH[@]}" --max-time 300 --ftp-create-dirs -T "$f" "$DEST/$enc" -w "%{http_code}")
            else
                variant="plain"
                code=$(curl -s "${CURL_AUTH[@]}" --max-time 300 -T "$f" "$DEST/$enc" -w "%{http_code}")
            fi
            # Confirm with a targeted SIZE, not the upload's return code and not a directory
            # listing. The return code lied once (FTP 200 on a file that landed at zero bytes) and
            # the listing lies whenever sys-ftpd is busy -- which is how the previous version of
            # this loop burned three attempts each on 36 files that were already correct.
            got=$(curl -sI "${CURL_AUTH[@]}" --max-time 45 "$DEST/$enc" 2>/dev/null \
                  | tr -d '\r' | awk '/^Content-Length:/ { print $2; exit }')
            if [ "$got" = "$want" ]; then echo "ok ($code, $variant)"; ok=1; break; fi
            # SIZE CAN BE WRONG ABOUT A FILE THAT IS RIGHT, so a disagreement is settled by
            # reading the bytes before it is called a failure.
            #
            # `fighter/pikachu/model/cshot/c00/model.xmb` burned twelve attempts across four
            # deploys on this. Every `-T` returned 226; `RETR` read back all 1108 bytes and
            # byte-matched the local file; `LIST` showed 1108 -- and `SIZE`, which is all this
            # loop consulted, kept answering 0. The card was correct the whole time and the
            # deploy kept re-uploading over a correct file and calling itself failed.
            #
            # Deleting first was tried and is WORSE: the DELE lands, the very next `-T` in the
            # same loop does not, and the file ends up genuinely absent. So the fix is not to
            # write harder, it is to stop trusting one reading. A RETR costs a transfer, but only
            # on a file SIZE has already disputed -- which is rare, and is exactly when being
            # sure is worth a second of transfer.
            tmpv=$(mktemp)
            if curl -s "${CURL_AUTH[@]}" --max-time 120 "$DEST/$enc" -o "$tmpv" 2>/dev/null                && cmp -s "$tmpv" "$f"; then
                echo "ok ($code, $variant; SIZE said ${got:-absent}, the bytes match)"
                rm -f "$tmpv"; ok=1; break
            fi
            rm -f "$tmpv"
            echo "still wrong (${got:-absent}, $variant, put=$code)"
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
