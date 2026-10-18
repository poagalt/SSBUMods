#!/usr/bin/env bash
# Full offline parity harness. Run before every install.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
M="$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/c00/motion_list.bin"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
rc=0

# PER-GATE TIMING, BECAUSE THIS HARNESS'S OWN HEADER WAS OUT BY A FACTOR OF FIFTEEN.
#
# tools/cached.sh still says "tools/check.sh takes 53 seconds and three steps are 50 of them".
# A measured deploy on 2026-09-20 spent 780.4s in this script alone -- the deploy's own `mark`
# line said so -- and nothing here could say WHERE, because a harness with 34 gates and no clock
# can only be profiled by rerunning it with a stopwatch.
#
# So every gate prints what it cost, on its own header line. A gate that grows a tree walk shows
# up the next time somebody reads the log instead of the next time somebody is annoyed enough to
# measure. The old second number -- elapsed-since-the-last-gate -- is gone: the gates no longer
# run one after another, so a column of deltas that does not sum to the total would be a lie
# printed 34 times.
CHECK_T0=$(date +%s%N)

# The three checks that cost 50 of this harness's 53 seconds go through tools/cached.sh, which
# re-runs them only when something they could read has changed. `--all` forces all of them.
[ "${1:-}" = "--all" ] && export CHECK_NOCACHE=1

# SHARED STATE, HOISTED, BECAUSE THE GATES NO LONGER SHARE A SHELL.
#
# Each gate below runs in its own subshell, so a variable one gate sets is invisible to the next.
# That is the point, and it is also the one thing the old linear script quietly leaned on.
# Exactly five assignments crossed a gate boundary, all of them set at the top of 3b: 3f reads
# $A, $clone_rows and $clone_on, and 3g reads $clone_rows. (3g was the one nobody remembered --
# it derives its article-path exemption list from the same table, which is why a hoist and not a
# hand-off between two named gates is the right shape here.)
#
# Deriving them once up here is the same answer for one grep and one sed: both inputs are files
# this harness only ever READS, so there is no run order in which they could differ. $V and
# $clone_live are used only inside 3b; they ride along so the whole clone picture is worked out
# in one place instead of half here and half three gates down.
V="$ROOT/pikachu-moveset/mod/fighter/pikachu/param/vl.prc"
A="$ROOT/pikachu-moveset/plugin/src/articles/mod.rs"
clone_on=$(grep -cx 'pub const CLONE_ENABLED: bool = true;' "$A" || true)
# The article is no longer the spikeball, and there is no longer only one. src/articles/mod.rs
# holds a candidate table with a per-row `enabled`, so the param keys to look for are derived from
# those names rather than written here -- `param_spikeball` hashed to 0x000f679aaf13, which is what
# this used to hardcode, and every other article has a different key.
# THE DONOR AND ITS ARTICLE NAME RIDE ALONG IN FIELDS 5 AND 6, so the "is this asset really
# missing" test further down can ask the archive about the right path. Nothing needed them
# before; Samus's model-less charge shot is what did.
clone_rows=$(sed -n 's/.*Candidate *{ *host_name: *"\([^"]*\)".*donor: *"\([^"]*\)".*donor_article: *"\([^"]*\)".*clean: *\([a-z]*\) *, *enabled: *\([a-z]*\) *, *authored: *\([a-z]*\).*/\1|\4|\5|\6|\2|\3/p' "$A")
clone_live=$(printf '%s\n' "$clone_rows" | awk -F'|' '$2=="true" && $3=="true" {print $1}')

# RUNNING THE GATES CONCURRENTLY, BECAUSE ALMOST NONE OF THIS IS COMPUTATION.
#
# `bash -x tools/check-weapons.sh` counts ~760 process spawns against 10.5s of user CPU, and a
# process spawn under Git Bash on Windows measures 50-70ms -- 50 `jq` spawns against a one-key
# document take 2.5s. So the minutes this harness costs are minutes spent BLOCKED in
# CreateProcess, not minutes spent computing, on a machine with 24 cores. Blocked work is the one
# kind that parallelises perfectly: the floor is the slowest single gate, 4k (check-weapons.sh)
# at ~148s of a ~364s serial total.
#
# WHAT MAKES IT SAFE.
#   * Every gate runs in its own SUBSHELL, with stdout and stderr redirected to its own file under
#     work/.gates. Nothing a gate does to its shell -- a variable, a cd, a trap, a mktemp -- can
#     reach another gate, and no two gates can interleave a line into the same stream.
#   * The five variables that genuinely crossed a gate boundary are hoisted above, derived once.
#     There are no others; every body was scanned for a top-level assignment read by a later one.
#   * No two gates that can run at the same time write the same path. Each tools/check-*.sh is
#     named by exactly one gate, so the fixed scratch directories three of them use
#     (work/bonecheck, work/checkanim, work/luaconst) have exactly one writer and the rest use
#     mktemp; the three tools/cached.sh calls pass three distinct cache names; /tmp/_rt.json
#     belongs to gate 3 alone.
#   * The log is REPLAYED in the original order, gate by gate, as each result lands. These gates
#     explain themselves at length and their order is part of the explanation, so what a person
#     reads is the same log as before -- only the wall clock changed.
#
# THE ONE BEHAVIOUR THIS CHANGES, and it is worth knowing about. A backgrounded command in a
# non-interactive shell gets its stdin from /dev/null, so every gate now reads EOF instead of
# inheriting the terminal. No gate wants to read stdin, so nothing is lost -- but a gate that
# reads it BY ACCIDENT used to hang the whole harness and now returns empty instead. That is not
# hypothetical: `awk '...' $(grep -rl ...)` in tools/check-victim.sh passes awk no file operands
# when the grep finds nothing, and on 2026-09-20 it found nothing, because the function it looks
# for had just been renamed. The serial harness sat on that awk until it was killed. Being harder
# to hang is a nice accident, not a licence: a gate that needs stdin has to say so itself.
#
# TWELVE AT A TIME, NOT THIRTY-FOUR. Several gates walk vendor/SSBU-Dumped-Scripts, which is
# 38,111 files. Past about a dozen concurrent readers the disk is the queue: another core buys
# nothing and every gate already in flight gets slower. CHECK_JOBS overrides it.
#
# CHECK_SERIAL=1 IS CHECK_JOBS=1 -- same subshells, same capture, same replay, one gate at a time
# in order. One code path, deliberately: a debugging mode that runs the gates differently can
# only tell you about itself. Reach for it when a gate starts failing in a way that smells like
# interference; if it passes at one and fails at twelve, the interference is real and the
# paragraph above is wrong somewhere.
CHECK_JOBS=${CHECK_JOBS:-12}
[ "${CHECK_SERIAL:-0}" = "1" ] && CHECK_JOBS=1
case "$CHECK_JOBS" in ''|*[!0-9]*|0) CHECK_JOBS=12 ;; esac

# Wiped at the start of every run so a stale .done from a killed run can never be replayed as
# this run's answer. Kept afterwards: when a gate fails, work/.gates/<n>.out is that gate and
# nothing else, which is a better thing to page through than a 34-gate log.
#
# This `rm -rf` is the only destructive thing this harness does, and it is one failed `cd` away
# from being rooted at "/". So the path is proved to be this repo before anything is removed.
[ -d "$ROOT/tools" ] && [ -d "$ROOT/pikachu-moveset" ] \
    || { echo "!! ROOT=$ROOT is not the repo; refusing to run"; exit 2; }
GATE_DIR="$ROOT/work/.gates"
rm -rf "$GATE_DIR"; mkdir -p "$GATE_DIR"

# ADDING A GATE IS A LOCAL EDIT. Run order is the order these `gate_def` lines execute, and a
# gate's function is named after its own label -- NOT after its position -- so a gate dropped into
# the middle renumbers nothing. That was the first version's mistake: with gate_01..gate_34 named
# by position, inserting 3f3 after 3f2 meant renaming twenty-six functions to add five lines.
#
# One new gate is one `gate_def` line plus the body it names, pasted where it belongs in the
# order, and nothing else in this file changes:
#
#     gate_def gate_3f3 "3f3. a cloned article ships its whole archive directory"
#     gate_3f3() {
#     local rc=0
#     bash "$ROOT/tools/check-articlefiles.sh" || rc=1
#     return $rc
#     }
#
# `local rc=0` first and `return $rc` last are what carry a body's `rc=1` out of its subshell;
# everything between them is an ordinary gate body, at column 0 (see below).
declare -a GATE_TITLES=() GATE_FNS=()
gate_def() { GATE_FNS+=("$1"); GATE_TITLES+=("$2"); }

# One gate, run and captured. `.done` is written LAST, and the replay loop waits on it rather than
# on the pid, so a half-written .out can never be printed as a finished gate. The capture files
# are named "<position>-<function>", so `ls work/.gates` after a failure reads as the run order
# and says which gate each file is without counting.
run_gate() {
    local i="$1" slot t0 t1
    printf -v slot '%02d-%s' "$((i + 1))" "${GATE_FNS[$i]}"
    printf '%s' "$slot" > "$GATE_DIR/$i.slot"
    t0=$(date +%s%N)
    ( "${GATE_FNS[$i]}" ) > "$GATE_DIR/$slot.out" 2>&1
    printf '%s' "$?" > "$GATE_DIR/$i.rc"
    t1=$(date +%s%N)
    printf '%s' "$(( (t1 - t0) / 1000000 ))" > "$GATE_DIR/$i.ms"
    : > "$GATE_DIR/$i.done"
}

# ...and the same gate printed where it would have printed if this were still one long script.
# The blank line and the `== ... ==` header are what the old gate() emitted, character for
# character, so the two logs diff clean with only the timing stripped.
replay_gate() {
    local i="$1" grc ms slot
    grc=$(cat "$GATE_DIR/$i.rc" 2>/dev/null || echo 1)
    ms=$(cat "$GATE_DIR/$i.ms" 2>/dev/null || echo 0)
    slot=$(cat "$GATE_DIR/$i.slot" 2>/dev/null || echo "$i")
    echo
    printf '== %s ==  [%5.1fs]\n' "${GATE_TITLES[$i]}" "${ms}e-3"
    cat "$GATE_DIR/$slot.out" 2>/dev/null
    # Same arithmetic the old script did inline: rc is the OR of every gate's status. A body still
    # signals failure by assigning rc=1; `local rc=0` and `return $rc` around each one is what
    # carries that out of the subshell, so not one body needed editing to get here.
    [ "$grc" = "0" ] || rc=1
}

# THE GATES. Each is the body it always was, wrapped in a function, with a `gate_def` on the line
# the `gate "..."` call used to occupy, so a gate is still one block you read top to bottom.
#
# NOTHING INSIDE A FUNCTION IS INDENTED, including `local rc=0` and `return $rc`. That is not
# sloppiness: several bodies contain heredocs whose terminator has to sit at column 0, and
# indenting the block to make it look like a function is a syntax error. The bodies are verbatim.
#
# A GATE'S FUNCTION IS NAMED AFTER ITS LABEL, so two gates may not share one. `3d` was
# duplicated once -- config.json's paths and replaced moves -- and the second is `3d2` now.
# The generator refuses a collision rather than quietly dropping a gate.
gate_def gate_1 "1. hitbox bones exist on Pikachu"
gate_1() {
local rc=0
bash "$ROOT/tools/check-bones.sh" || rc=1
return $rc
}

gate_def gate_2 "2. every registered ACMD is reachable from a motion entry"
gate_2() {
local rc=0
"$ROOT/tools/motion-patch/target/release/checkacmd.exe" "$M" "$ROOT/pikachu-moveset/plugin/src" || rc=1
return $rc
}

gate_def gate_3 "3. shipped binaries reparse"
gate_3() {
local rc=0
for f in "$M" "$ROOT"/pikachu-moveset/mod/fighter/pikachu/model/body/c00/model.*; do
  case "$f" in *.nutexb|*.adjb) continue ;; esac
  n=$(basename "$f")
  if [ "$n" = "motion_list.bin" ]; then
    if "$ROOT/tools/motion-patch/target/release/motion-patch.exe" --help >/dev/null 2>&1 || true; then :; fi
    "$ROOT/tools/motion-patch/target/release/valcheck.exe" "$f" >/dev/null 2>&1 \
      && echo "  ok  $n" || { echo "  FAIL $n"; rc=1; }
  else
    "$T" "$f" /tmp/_rt.json >/dev/null 2>&1 && echo "  ok  $n" || { echo "  FAIL $n"; rc=1; }
  fi
done
return $rc
}

gate_def gate_3b "3b. vl.prc matches whether an article clone is installed"
gate_3b() {
local rc=0
# `param_spikeball` in Pikachu's vl.prc is only correct while clone_weapon is actually enabled.
# With the clone disabled it is dead config that still takes Pikachu from 20 article params to 21,
# declaring one with no weapon behind it -- and Training Modpack's save state iterates article
# indices calling get_article and remove_exist_object_id, which is a plausible crash on the 21st.
# So the two must agree in both directions.
# $V, $A, $clone_on, $clone_rows and $clone_live are derived once in the preamble -- 3f and 3g
# read them too, and the gates no longer share a shell. See the hoist comment up there.
k=0; pname=""
if [ -f "$V" ]; then
  for n in $clone_live; do
    key=$(bash "$ROOT/tools/hash40.sh" "param_$n" 2>/dev/null | awk '{print $1}' | head -1)
    kp=$(printf '0x%012x' "$key" 2>/dev/null || echo "")
    [ -n "$kp" ] || continue
    if [ "$("$ROOT/tools/prc-tool/target/release/vlkeys.exe" "$V" 2>/dev/null | grep -c "$kp" || true)" != "0" ]; then
      k=$((k+1)); pname="$pname param_$n"
    fi
  done
fi
nlive=$(printf '%s\n' "$clone_live" | grep -c . || true)
[ -n "$pname" ] || pname="param_<article>"
# THE CLONE DOES NOT REQUIRE param_spikeball, and this check used to insist that it did.
#
# The claim was that smashline null-dereferences without it. Reading vendor/sl_params.rs settles
# it: get_param_reimpl looks the key up in the filtered map and, on a miss, calls
# `(data.original_lookup_param.unwrap())(object, key)` -- it falls through to the fighter's own
# param holder. A missing article param means the spikeball runs on fall-through values, not a
# crash. And the crash it was blamed for was in load_fighter_effects, a different function that
# handles effect search indices and never touches params.
#
# So shipping the param object is an IMPROVEMENT (real values instead of fall-through), not a
# prerequisite -- which is what makes the clone testable without modifying vl.prc at all, and
# therefore testable as a single variable.
if [ "$clone_on" = "0" ] && [ "$k" != "0" ]; then
  echo "  FAIL clone_weapon is disabled but vl.prc still ships$pname -- dead article param"; rc=1
elif [ "$clone_on" = "0" ]; then
  echo "  ok  clone disabled, vl.prc ships no article param"
elif [ "$k" = "0" ]; then
  echo "  note $nlive article(s) enabled with no vl.prc article param -- they use fall-through values"
elif [ "$k" != "$nlive" ]; then
  echo "  note vl.prc declares$pname but $nlive article(s) are enabled -- the rest fall through"
else
  echo "  ok  clone enabled and vl.prc declares a param for each:$pname"
fi

# THE OVERRIDE MUST NOT CHANGE THE ARTICLE COUNT, and it must actually be Incineroar's
# param_special_s rather than Pikachu's.
#
# The two are separate failures with the same symptom. Vanilla Pikachu has 20 top-level vl.prc
# entries; gen-vlprc.sh REPLACES param_special_s rather than adding one, so the count must stay 20
# -- 21 would mean an article param crept in, which is what Training Modpack's save state walks.
# And if the replace silently skipped (vlmerge without --replace just prints and returns), the
# object would still be Skull Bash's, every through_target lookup the victim's status makes would
# return 0, and the whipped opponent would go nowhere. Checking a key that only Incineroar's object
# has catches that.
# The param_special_s override and what is shipped must agree in BOTH directions. It is gated off
# in tools/gen-vlprc.sh because shipping it null-dereferenced smashline's load_fighter_effects on
# fighter load -- so a check that only fired when the file was PRESENT would have missed the case
# that actually happened.
want_ss=$(grep -cx 'SHIP_PARAM_SPECIAL_S=1' "$ROOT/tools/gen-vlprc.sh" || true)
# The whip's ledge box (2026-10-12) is the other reason gen-vlprc.sh ships a vl.prc. It follows the
# plugin's WHIP_CLIFF_HANGDATA rather than a flag of its own, so read the same line it reads.
want_whip=$(grep -cx 'pub const WHIP_CLIFF_HANGDATA: bool = true;' \
            "$ROOT/pikachu-moveset/plugin/src/specials/whip/mod.rs" || true)
want_vl=$(( want_ss + want_whip ))
if [ "$want_vl" != "0" ] && [ ! -f "$V" ]; then
  echo "  FAIL gen-vlprc.sh ships a vl.prc (param_special_s=$want_ss whip ledge box=$want_whip) but none is present"; rc=1
elif [ "$want_vl" = "0" ] && [ -f "$V" ]; then
  echo "  FAIL a vl.prc is shipped but the override is disabled -- delete it or re-enable the gate"; rc=1
elif [ "$want_vl" = "0" ]; then
  echo "  ok  param_special_s override off, no vl.prc shipped (vanilla, no load-time risk)"
else
  n=$("$ROOT/tools/prc-tool/target/release/vlkeys.exe" "$V" 2>/dev/null | grep -c '^ *0x' || true)
  if [ "$n" != "20" ]; then
    echo "  FAIL shipped vl.prc has $n top-level entries, expected 20 -- something was ADDED, not replaced"; rc=1
  fi
  if [ "$want_ss" != "0" ]; then
    tts=$("$ROOT/tools/prc-tool/target/release/prcdump.exe" "$V" \
            "$ROOT/vendor/param-labels/ParamLabels.csv" 2>/dev/null \
          | grep -c 'special_s_through_target_speed = 11.2' || true)
    if [ "$tts" = "0" ]; then
      echo "  FAIL shipped vl.prc param_special_s is not Incineroar's -- the rope throw will not travel"; rc=1
    else
      echo "  ok  vl.prc carries Incineroar's param_special_s ($n entries, article count unchanged)"
    fi
  fi
  if [ "$want_whip" != "0" ]; then
    # Pikachu's own [0] plus Incineroar's SPECIAL_S box as [1] -- the index the plugin selects.
    nch=$("$ROOT/tools/prc-tool/target/release/prcdump.exe" "$V" \
            "$ROOT/vendor/param-labels/ParamLabels.csv" cliff_hang_data --depth 2 2>/dev/null \
          | grep -c 'cliff_hang_data list\[2\]' || true)
    if [ "$nch" = "0" ]; then
      echo "  FAIL WHIP_CLIFF_HANGDATA is on but the shipped vl.prc's cliff_hang_data is not list[2]"; rc=1
    else
      echo "  ok  vl.prc carries the whip's ledge box at cliff_hang_data[1] ($n entries)"
    fi
  fi
fi
return $rc
}

gate_def gate_3e2 "3e2. the generated param provider still agrees with Incineroar's prc"
gate_3e2() {
local rc=0
# WHAT src/params ANSWERS WITH, CHECKED AGAINST WHERE IT CAME FROM.
#
# `src/params/table.rs` is generated by tools/gen-paramprovider.sh straight out of
# `extracted/fighter/gaogaen/param/vl.prc`, so this is not checking that somebody typed fifty
# floats carefully -- it is checking that the GENERATOR RAN, and that the prc it ran against has
# not changed underneath the committed output.
#
# It used to check twelve hand-typed rows, being the twelve the VICTIM's status reads. The
# transcribed whip reads eight more in `FUN_710001d4b0` alone -- the kinetic setup all six of
# Incineroar's side-B statuses call -- and a transcription running on a Pikachu reads those off
# PIKACHU, where every one comes back 0. So the provider answers the whole object and the count is
# whatever the prc has.
TABLE="$ROOT/pikachu-moveset/plugin/src/params/table.rs"
GPRC="$ROOT/extracted/fighter/gaogaen/param/vl.prc"
if [ ! -f "$TABLE" ]; then
  echo "  FAIL src/params/table.rs is missing -- run bash tools/gen-paramprovider.sh"; rc=1
elif [ ! -f "$GPRC" ]; then
  echo "  FAIL no extracted gaogaen vl.prc to check the generated table against"; rc=1
else
  gdump=$("$ROOT/tools/prc-tool/target/release/prcdump.exe" "$GPRC" \
            "$ROOT/vendor/param-labels/ParamLabels.csv" 2>/dev/null \
          | awk 'index($0,"  param_special_s ")==1{f=1;next} f&&/^  [a-z0-9_]+ /{f=0} f' \
          | awk '/^    \[0\]/{p=1;next} /^    \[[1-9]\]/{p=0} p')
  bad=0; rows=0
  while read -r value name; do
    [ -z "${name:-}" ] && continue
    rows=$((rows+1))
    got=$(printf '%s\n' "$gdump" | awk -v k="$name" '$1==k { print $3; exit }')
    if [ -z "$got" ]; then
      echo "  FAIL table.rs names $name, which is not in gaogaen's param_special_s"; bad=1
    elif [ "$got" != "$value" ]; then
      echo "  FAIL table.rs has $name = $value but the prc says $got"; bad=1
    fi
  done <<EOF
$(sed -n 's/^    (0x[0-9a-f_]*, \([-0-9.]*\), "\([a-z0-9_]*\)"),$/\1 \2/p' "$TABLE")
EOF
  if [ "$rows" -lt 40 ]; then
    echo "  FAIL the table has only $rows row(s); the generator should emit the whole object"; rc=1
  elif [ "$bad" = "0" ]; then
    echo "  ok  all $rows generated params match gaogaen's own param_special_s"
  else
    rc=1
  fi
fi
return $rc
}


gate_def gate_3d "3d. config.json declares exactly the paths it should, under the right keys"
gate_3d() {
local rc=0
# THE FILE MOST IMPLICATED IN THIS PROJECT'S LOAD HANGS, and until now nothing checked it.
#
# Its own generator's header names the failure modes: "an undeclared file simply never loads, and
# a declared file that is absent injects a bogus entry into the search filesystem, which is a
# plausible cause of the load crashes this project has been chasing". Both are checked here, plus
# two more that were each found the hard way.
#
#   * A DECLARED PATH THAT VANILLA ALREADY HAS is not a new addition, it is a same-name
#     replacement, and declaring it attaches a duplicate entry to a directory that already lists
#     it. 308 of 577 declarations were in this state the moment the mod began shipping more than
#     one costume, because gen-config.sh was asking `extracted/` -- a PARTIAL mirror with 5 of
#     c01's 338 motion files -- instead of the archive map.
#
#   * A PATH DECLARED UNDER THE WRONG COSTUME KEY is on the card, declared, verified by size, and
#     invisible to the costume it belongs to. `new-dir-files` maps a DIRECTORY to files to add to
#     it, and a c05 Pikachu loads `fighter/pikachu/c05`; 210 c01..c07 paths were declared under
#     the `c00` key.
#
# The archive map is the oracle where available, for the same reason gen-config.sh uses it.
CFG="$ROOT/pikachu-moveset/mod/config.json"
ARCMAP="$ROOT/work/acquire/arc-paths.txt"
if [ ! -f "$CFG" ]; then
  echo "  FAIL no config.json -- run tools/gen-config.sh"; rc=1
elif ! jq -e . "$CFG" >/dev/null 2>&1; then
  echo "  FAIL config.json is not valid JSON"; rc=1
else
  # EVERY `jq -r` HERE IS PIPED THROUGH `tr -d '\r'`, and leaving it out made this gate lie.
  # This jq writes CRLF, so a path comes back as `...nutexb<CR>`, `[ -f ]` looks for a file whose
  # name ends in a carriage return and says no -- reporting all 268 shipped files as absent -- and
  # the two `comm` checks below found no overlap with anything and reported OK. A gate that fails
  # on a correct tree and passes on a broken one is worse than no gate. See the long note in
  # tools/ship-textures.sh.
  ndecl=$(jq -r '[."new-dir-files"[]?[]]|length' "$CFG" | tr -d '\r')
  # 1. every declared path is a real file in the tree
  absent=$(jq -r '."new-dir-files"[]?[]' "$CFG" | tr -d '\r' | while read -r p; do
             [ -f "$ROOT/pikachu-moveset/mod/$p" ] || echo "$p"; done)
  if [ -n "$absent" ]; then
    echo "  FAIL declared but NOT shipped (bogus search-filesystem entries):"
    printf '       %s\n' $absent | head -10; rc=1
  else
    echo "  ok  all $ndecl declared path(s) are real files in the mod tree"
  fi
  # 2. every declared path sits under the costume its key names
  wrongkey=$(jq -r '."new-dir-files"|to_entries[]|.key as $k|($k|sub("fighter/pikachu/";"")) as $c
                    |.value[]|select((.|test("/"+$c+"/"))|not)|"\($k) <- \(.)"' "$CFG" | tr -d '\r')
  if [ -n "$wrongkey" ]; then
    echo "  FAIL declared under a key whose costume it does not belong to:"
    printf '       %s\n' "$wrongkey" | head -10; rc=1
  else
    echo "  ok  every declared path lives under its key's costume directory"
  fi
  # 3+4. nothing over- or under-declared, against the real archive
  if [ -s "$ARCMAP" ]; then
    van=$(mktemp); dec=$(mktemp); shp=$(mktemp)
    grep '^fighter/pikachu/' "$ARCMAP" | sort -u > "$van"
    jq -r '."new-dir-files"[]?[]' "$CFG" | tr -d '\r' | sort -u > "$dec"
    ( cd "$ROOT/pikachu-moveset/mod" && find . -type f | sed 's|^\./||' ) \
      | grep '^fighter/pikachu/' | sort -u > "$shp"
    over=$(comm -12 "$dec" "$van")
    under=$(comm -23 "$shp" "$van" | comm -23 - "$dec")
    if [ -n "$over" ]; then
      echo "  FAIL declared but vanilla ALREADY has it (same-name replacement, needs no entry):"
      printf '       %s\n' $over | head -6; rc=1
    else
      echo "  ok  no declaration duplicates a path vanilla already has"
    fi
    if [ -n "$under" ]; then
      echo "  FAIL shipped, absent from vanilla, and NOT declared -- it will never load:"
      printf '       %s\n' $under | head -6; rc=1
    else
      echo "  ok  every shipped path vanilla lacks is declared"
    fi
    rm -f "$van" "$dec" "$shp"
  else
    echo "  ..  no archive map at work/acquire/arc-paths.txt -- over/under-declaration unchecked"
  fi
fi
return $rc
}

gate_def gate_3f "3f. a cloned article ships its own model and motion"
gate_3f() {
local rc=0

# THIS IS THE CHECK THAT WOULD HAVE PREVENTED A CRASH. The clone was enabled with NONE of the
# spikeball's assets in the tree -- config.json never mentioned spikeball at all -- and loading into
# a match died in the game's own weapon-creation path. A cloned weapon is owned by pikachu, so the
# game looks for `fighter/pikachu/{model,motion}/spikeball/c00/...`, and there was nothing there.
#
# tools/gen-article-assets.sh copies or removes them to match CLONE_ENABLED, but nothing forced
# anyone to RUN it. This does: the tree and the constant must agree in both directions, because
# leftover assets with the clone off are dead declared paths in the search filesystem and missing
# assets with the clone on are a null dereference on load.
#
# IT ALSO COVERS EVERY OTHER CANDIDATE, not just the enabled ones. The table in
# src/articles/mod.rs lists seven articles with a per-row `enabled`; assets left behind from a row
# that was turned off are dead declared paths, which is the same failure as never shipping them.
cands=$(sed -n 's/.*Candidate *{ *host_name: *"\([^"]*\)".*/\1/p' "$A")
# The slot set gen-article-assets.sh would ship, honouring the same env override it does.
art_slots="${ARTICLE_SLOTS:-$(sed -n 's/^ARTICLE_SLOTS="\${ARTICLE_SLOTS:-\(.*\)}"$/\1/p' "$ROOT/tools/gen-article-assets.sh" | head -1)}"
[ -n "$art_slots" ] || art_slots=c00
echo "  ARTICLE_SLOTS=$art_slots"
for name in $cands; do
  for kind in model motion; do
    d="$ROOT/pikachu-moveset/mod/fighter/pikachu/$kind/$name"
    n=0; [ -d "$d" ] && n=$(find "$d" -type f | wc -l)
    row=$(printf '%s\n' "$clone_rows" | awk -F'|' -v t="$name" '$1==t')
    a_clean=$(printf '%s' "$row" | cut -d'|' -f2); a_en=$(printf '%s' "$row" | cut -d'|' -f3)
    want_on=0
    [ "$clone_on" != "0" ] && [ "$a_en" = "true" ] && [ "$a_clean" = "true" ] && want_on=1
    # A DIRTY ROW THAT src/articles/neutralize.rs COVERS IS INSTALLED TOO -- the same test
    # gen-article-assets.sh ships it by, so the two cannot disagree about the rope.
    if [ "$clone_on" != "0" ] && [ "$a_en" = "true" ] && [ "$a_clean" = "false" ]        && grep -q "(\"$name\", &\[" "$ROOT/pikachu-moveset/plugin/src/articles/neutralize.rs"; then
      want_on=1
    fi
    # AN ARTICLE THE ARCHIVE HAS NO `model` FOR IS NOT A MISSING ASSET, and treating it as one
    # is what this reported on Samus's charge shot. `fighter/samus/model/cshot` is not a path in
    # the 590,711-path archive map at all -- only `motion/cshot/cNN/motion_list.bin` is --
    # because the ball she fires is made entirely of effects. Asking the map is the same test
    # gen-article-assets.sh makes before skipping the directory, so the two agree by
    # construction rather than by someone remembering. Without the map the old hard error
    # stands, because then "absent locally" and "absent from the game" cannot be told apart.
    a_donor=$(printf '%s' "$row" | cut -d"|" -f5); a_dart=$(printf '%s' "$row" | cut -d"|" -f6)
    ARC_PATHS="$ROOT/work/acquire/arc-paths.txt"
    if [ "$want_on" = "1" ] && [ "$n" = "0" ] && [ -s "$ARC_PATHS" ] && [ -n "$a_donor" ] \
       && ! grep -q "^fighter/$a_donor/$kind/$a_dart/" "$ARC_PATHS"; then
      echo "  -   $kind/$name: the archive has no $kind for $a_donor/$a_dart, so there is none to ship"
    elif [ "$want_on" = "1" ] && [ "$n" = "0" ]; then
      echo "  FAIL article '$name' is enabled but $kind/$name is empty -- run tools/gen-article-assets.sh"; rc=1
    elif [ "$want_on" = "0" ] && [ "$n" != "0" ]; then
      echo "  FAIL $kind/$name holds $n file(s) but '$name' is not installed -- run tools/gen-article-assets.sh"; rc=1
    elif [ "$want_on" = "1" ]; then
      s=$(find "$d" -mindepth 1 -maxdepth 1 -type d | wc -l)
      echo "  ok  clone targets '$name', $kind/$name ships $n file(s) across $s costume slot(s)"
      # AND THE SLOT SET MUST BE THE ONE ASKED FOR. `ARTICLE_SLOTS` in gen-article-assets.sh
      # decides it -- c00 for iterating, `all` for a release -- and the two failure directions are
      # both real: fewer slots than intended is a cNN Pikachu with no article model at all, and
      # more is seven eighths of a twelve-minute deploy nobody wanted. Read from the generator so
      # there is one declaration, with the same env override it honours.
      got_slots=$(find "$d" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort | tr '\n' ' ')
      if [ "$art_slots" != "all" ]; then
        exp_slots=$(printf '%s\n' $art_slots | sort | tr '\n' ' ')
        if [ "$got_slots" != "$exp_slots" ]; then
          echo "  FAIL $kind/$name ships slots [$got_slots] but ARTICLE_SLOTS asks for [$exp_slots] -- run tools/gen-article-assets.sh"; rc=1
        fi
      fi
      # AND EVERY BYTE MUST BE THE ARC'S. gen-article-assets.sh copies verbatim -- nothing about a
      # cloned article is generated or rewritten -- so any difference means a truncated transfer or
      # a hand edit, and this catches both without a size heuristic. It is also what lets 3g above
      # let a 96-byte motion_list.bin through.
      # AUTHORED ARTICLES HAVE NO ARC COPY TO BE IDENTICAL TO. Half their files are assembled
      # from a donor's body model, so byte equality is the wrong question, and
      # `author-article.py --verify` asks the right ones instead -- see section 3f2 below.
      a_authored=$(printf '%s' "$row" | cut -d'|' -f4)
      if [ "$a_authored" = "true" ]; then
        echo "  ok  '$name' is authored -- byte-identity not applicable, see 3f2"
        continue
      fi
      a_donor=$(sed -n "s/.*Candidate *{ *host_name: *\"$name\" *, *donor: *\"\([^\"]*\)\".*/\1/p" "$A" | head -1)
      a_art=$(sed -n "s/.*Candidate *{ *host_name: *\"$name\" *, *donor: *\"[^\"]*\" *, *donor_article: *\"\([^\"]*\)\".*/\1/p" "$A" | head -1)
      # A LENT MODEL IS COMPARED AGAINST WHAT IT WAS LENT FROM.
      #
      # `model_from` in the Candidate row names a `<fighter>/<dir>` whose MODEL stands in for a
      # donor that has none. Exactly one row uses it: Samus's charge shot has no model anywhere in
      # the archive -- the ball is made of her effects, which ADR 0002 excludes -- so it borrows
      # Pikachu's own electro ball. Resolving its files against the donor made this gate report
      # eleven "has no source at extracted/fighter/samus/model/cshot/...", which is the gate being
      # right about the old assumption and wrong about the new one.
      #
      # MOTION IS NEVER LENT. The weapon's own behaviour lives there, and the whole point of the
      # substitution is that only the geometry changes -- the article is still
      # WEAPON_KIND_SAMUS_CSHOT and still flies in Samus's straight line.
      a_mf=$(sed -n "s/.*Candidate *{ *host_name: *\"$name\" *,.*model_from: *\"\([^\"]*\)\".*/\1/p" "$A" | head -1)
      a_rel="$a_donor/$kind/$a_art"
      if [ "$kind" = "model" ] && [ -n "${a_mf:-}" ]; then
        a_rel="${a_mf%%/*}/model/${a_mf#*/}"
      fi
      # Same two source roots, same order, as tools/gen-article-assets.sh: work/arc-vis holds the
      # complete sets pulled off the running game, extracted/ is the older partial copy.
      a_src=""
      for r in "$ROOT/work/arc-vis/fighter" "$ROOT/extracted/fighter"; do
        if [ -d "$r/$a_rel" ] && [ -n "$(ls -A "$r/$a_rel" 2>/dev/null)" ]; then
          a_src="$r/$a_rel"; break
        fi
      done
      [ -n "$a_src" ] || a_src="$ROOT/extracted/fighter/$a_rel"
      diffs=0
      while IFS= read -r f; do
        rel=${f#"$d/"}
        if [ ! -f "$a_src/$rel" ]; then
          echo "  FAIL $kind/$name/$rel has no source at ${a_src#"$ROOT/"}/$rel"; rc=1; diffs=$((diffs+1))
        elif ! cmp -s "$f" "$a_src/$rel"; then
          echo "  FAIL $kind/$name/$rel differs from the arc's copy -- rerun tools/gen-article-assets.sh"; rc=1
          diffs=$((diffs+1))
        fi
      done < <(find "$d" -type f)
      [ "$diffs" = "0" ] && echo "  ok  all $n file(s) byte-identical to ${a_src#"$ROOT/"}"
    fi
  done
done
[ "$clone_on" = "0" ] && echo "  ok  clone disabled, no article assets shipped"
return $rc
}

gate_def gate_3f2 "3f2. an authored article matches the donor's own geometry bit for bit"
gate_3f2() {
local rc=0
# Palutena's staff and Corrin's Yato are not articles in the game -- they are meshes inside their
# owner's body model -- so tools/author-article.py builds the article they never had. That is the
# only place in this mod where a Nintendo-derived binary is ASSEMBLED rather than copied, which is
# exactly why it earns a gate of its own: the skeleton must be socket(identity) -> bind bone with
# the donor's own transform, every vertex must be bit-identical to the donor's body model, the
# bounding sphere must contain them, and every material and texture must resolve.
PYX=""
for c in py python3 python; do command -v "$c" >/dev/null 2>&1 && { PYX="$c"; break; }; done
if [ -n "$PYX" ]; then
  "$PYX" "$ROOT/tools/author-article.py" --verify || rc=1
else
  echo "  skip  no python on PATH"
fi
return $rc
}

# EVERY FILE THE ARCHIVE HAS, NOT JUST THE ONES WE HAPPEN TO HOLD. 3f above proves the shipped
# bytes match the source they were copied FROM, which is blind to a partial source -- and a
# partial source is what crashed the game on 2026-09-20: Byleth's bow shipped 1 of its 18 model
# files and the engine dereferenced what was not there. This counts against the archive map.
gate_def gate_3f3 "3f3. a cloned article ships its whole archive directory"
gate_3f3() {
local rc=0
bash "$ROOT/tools/check-articlefiles.sh" || rc=1
return $rc
}

gate_def gate_3g "3g. no shipped file is empty or implausibly small"
gate_3g() {
local rc=0
# A ZERO-BYTE FILE ON THE CARD WAS BLAMED ON A CLONED ARTICLE FOR TWO CRASHES.
# `motion/body/c00/d03speciallwstart.nuanmb` uploaded truncated to 0 bytes and stayed that way
# across every deploy, because the verifier of the day checked presence with a directory LISTING
# and sys-ftpd truncates those under load. tools/deploy.sh now verifies by size and repairs, so the
# remote side is covered -- this covers the LOCAL side, where a generator that writes an empty file
# would otherwise sail through every other check here.
#
# The floor is per-extension and deliberately loose: it is looking for zero and near-zero, not for
# a file that is merely smaller than its neighbours.
# ARTICLE FILES ARE EXEMPT, and that is not a loosening. 3f above asserts every byte of every
# article file against the arc's own copy, which is strictly stronger than a size floor -- and the
# arc's copies are legitimately tiny: Chrom's sword `motion_list.bin` is 96 bytes because it
# declares two motions against Pikachu's 43,428, the parachute's `model.nuanmb` is 104 bytes, and
# Mythra's esword `update.prc` is 96. The floor is hunting a TRUNCATED TRANSFER, which byte
# equality rules out completely. Applying it to files this project does not generate would only
# ever produce false alarms, and a checker whose failure mode is a false alarm trains people to
# ignore it.
art_names=$(printf '%s\n' "$clone_rows" | cut -d'|' -f1 | grep -v '^$' | paste -sd'|' -)
bad_small=0
while IFS=$'\t' read -r sz path; do
  rel=${path#"$ROOT/pikachu-moveset/mod/fighter/pikachu/"}
  if [ -n "$art_names" ] && printf '%s' "$rel" | grep -qE "^(model|motion)/($art_names)/"; then
    continue
  fi
  case "$path" in
    *.nuanmb|*.numdlb|*.numshb|*.nusktb|*.numatb|*.numshexb|*.bin|*.prc) floor=128 ;;
    *) floor=16 ;;
  esac
  if [ "$sz" -lt "$floor" ]; then
    echo "  FAIL ${path#"$ROOT/"} is $sz bytes (floor $floor)"; bad_small=$((bad_small+1)); rc=1
  fi
done < <(find "$ROOT/pikachu-moveset/mod" -type f -printf '%s\t%p\n')
[ "$bad_small" = "0" ] && echo "  ok  all $(find "$ROOT/pikachu-moveset/mod" -type f | wc -l) shipped files have plausible sizes"
return $rc
}

gate_def gate_3l "3l. the shipped animations fit the fighter's memory budget"
gate_3l() {
local rc=0
bash "$ROOT/tools/check-animsize.sh" || rc=1
return $rc
}

gate_def gate_3h "3h. the in-game self-test walks the list this build ships"
gate_3h() {
local rc=0
# The self-test's value is that it names motions the CONSOLE failed to resolve. A stale table would
# name motions the mod no longer ships -- reporting failures that are not real -- while saying
# nothing about ones it added, which is the worse half. tools/build.sh regenerates it before the
# compile; this catches a plugin built without running the build.
G="$ROOT/pikachu-moveset/plugin/src/telemetry/motions.rs"
want=$(grep -vE '^\s*#|^\s*$' "$ROOT/tools/ported-motions.txt" | cut -d: -f2 | sort -u)
have=$(grep -oE '^\s+"[a-z_0-9]+",' "$G" 2>/dev/null | tr -d ' ",' | sort -u)
if [ ! -f "$G" ]; then
  echo "  FAIL no motions.rs -- run tools/gen-selftest.sh"; rc=1
elif [ "$want" != "$have" ]; then
  echo "  FAIL motions.rs is stale -- run tools/gen-selftest.sh"
  diff <(echo "$want") <(echo "$have") | head -6 | sed 's/^/        /'
  rc=1
else
  # Every name it will ask the console about must be a name we actually put in motion_list.bin,
  # or an UNRESOLVED line on hardware would be the check's own bug rather than a real failure.
  # Step 3e already proves that: mlparity.exe reports "not comparable (we do not ship it)" per motion
  # and came back with none.
  echo "  ok  motions.rs matches ported-motions.txt ($(echo "$have" | wc -l | tr -d ' ') motions)"
fi
return $rc
}
gate_def gate_3d2 "3d2. no replaced move plays one of Pikachu's own animations"
gate_3d2() {
local rc=0
# The exhaustive version of the bug that made two of the three f-tilt angles keep his tail whip:
# a source fighter need not have every variant Pikachu has. Classifies each into REPOINTED (the
# shipped motion_list names a different file than vanilla), LOCKED OUT (only reachable through a
# status src/specials/lockout bounces out of), or GAP -- and only GAP fails.
VAR="$ROOT/tools/motion-patch/target/release/variants.exe"
# RUN ONCE, MATCHED IN BASH, and never piped into `grep -q`.
#
# `producer | grep -q PATTERN` is a SIGPIPE trap under `pipefail`, and in THIS direction it fails
# the dangerous way: grep exits the instant it matches, the producer is killed writing the rest,
# `pipefail` reports the producer's death, and the `if` concludes there was NO match. A real GAP
# would have been reported as a pass. tools/check-srcparams.sh had the same bug in the harmless
# direction and it cost a debugging round; this one would have cost a wrong answer.
variants=$("$VAR" "$ROOT/extracted" "$M" 2>/dev/null)
printf "%s
" "$variants" | tail -2
case "$variants" in
  *"GAP --"*)
    echo "  FAIL a replaced move still plays one of Pikachu's own animations"; rc=1 ;;
esac
# THE INVERSE, which is the check that had never existed: a motion the SOURCE has in a ported
# family and we have no entry for is a variant of the move that simply is not reproduced. That is
# how the airborne Alolan Whip and airborne Revenge went unnoticed -- six and three motions each,
# with the grounded animation and grounded hitboxes played in their place. Verified non-tautological
# by running it against vanilla Pikachu's list, where it reports all 22 synthesised entries.
case "$variants" in
  *"0 source motion"*) ;;
  *)
    echo "  FAIL a ported move has a variant on the source fighter that this moveset does not have"; rc=1 ;;
esac
return $rc
}

gate_def gate_3k "3k. no synthesised motion runs one of Pikachu's own scripts"
gate_3k() {
local rc=0
# THE BUG THAT MADE THE UP-B INVISIBLE, and it was thirty-eight entries wide.
#
# A motion entry names four scripts: game_, plus expression_/sound_/effect_. synth.rs repointed
# game_script to the source's and left the other three pointing at the TEMPLATE's -- so
# `special_hi_open`, cloned from a Quick Attack entry, named `expression_specialairhi1`, and
# Pikachu's SpecialAirHi1 expression script calls VisibilityModule::set_whole(false). The up-B's
# own logic was correct and measured working on hardware; he was hidden by his own script.
#
# Same mechanism put Pikachu's Thunder scripts on Revenge's stance and his Quick Attack effect
# script under Game & Watch's Fire, which is where the "quick attack arrows" came from.
#
# REPOINTED entries are exempt and must be: `attack_s4_s` legitimately names `game_attacks4`,
# because that is what vanilla Pikachu calls it and what our ACMD is registered as. Only the
# entries synth.rs CREATES have to name their own. The list comes from synth.rs's own table, so
# it cannot drift from what is actually synthesised.
SYNTH_MOTIONS=$(sed -n '/^const SYNTH/,/^\];/p' "$ROOT/tools/motion-patch/src/bin/synth.rs" \
                | grep -oE '^\s*\("[a-z_0-9]+"' | tr -d ' ("' | tr '\n' ' ')
# `--vanilla` IS WHAT MAKES THIS THE REAL TEST RATHER THAN A PROXY, and Samus's Charge Shot forced
# it. Eleven of her entries name scripts after a DIFFERENT motion IN HER OWN LIST --
# `special_n_s` names `game_specialnstart` -- so copying them verbatim, which is what synth.rs
# argues for, reported as eleven failures under "the script base must equal the entry's own name".
# With Pikachu's vanilla list to check against, a mismatch fails only when the name resolves to a
# script HE HAS, which is the sentence in this gate's title and the thing that hid the up-B.
VANILLA_ML="$ROOT/extracted/fighter/pikachu/motion/body/c00/motion_list.bin"
MLSHOW="$ROOT/tools/motion-patch/target/release/mlshow.exe"
"$MLSHOW" "$M" --all-scripts --vanilla "$VANILLA_ML" $SYNTH_MOTIONS 2>/dev/null | tail -13
"$MLSHOW" "$M" --all-scripts --vanilla "$VANILLA_ML" $SYNTH_MOTIONS >/dev/null 2>&1 \
  || { echo "  FAIL a synthesised motion runs one of pikachu's own scripts -- see mlshow output above"; rc=1; }
return $rc
}

# 3k guards the entries synth.rs CREATES and must exempt repointed ones. This covers the exemption:
# a ported move landing on a motion Pikachu already has runs HIS expression_ script, and nothing
# checked what those do. See the script's header and Part 8 of docs/the-article-path.md.
gate_def gate_3k2 "3k2. no ported motion falls through to an expression script that hides him"
gate_3k2() {
local rc=0
PY_BIN=""
for c in py python3 python; do command -v "$c" >/dev/null 2>&1 && { PY_BIN="$c"; break; }; done
if [ -n "$PY_BIN" ]; then
    "$PY_BIN" "$ROOT/tools/check-expressions.py" || rc=1
else
    echo "  !! no python on PATH -- expression fall-through is unchecked"; rc=1
fi
return $rc
}

gate_def gate_3e "3e. each ported move carries its source's whole motion entry"
gate_3e() {
local rc=0
# NOT JUST blend_frames, WHICH IS ALL THIS USED TO CHECK. The entry also carries `cancel_frame`
# (the FAF -- literally the frame data this project exists to reproduce), the intangibility window,
# `no_stop_intp` and the animation flag set. motion-patch patches all of them and nothing verified
# any of them; measured against vanilla the FAF patch alone moves 20 of the 46 comparable entries,
# by as much as 11 frames.
#
# blend_frames is the "seamless transition" requirement in numbers, and it was worth finding on its
# own: Palutena's catch_dash was snapping in over 0 frames instead of 3.
MOTIONS=$(grep -vE '^\s*#|^\s*$' "$ROOT/tools/ported-motions.txt" | tr '\n' ' ')
MLP="$ROOT/tools/motion-patch/target/release/mlparity.exe"
"$MLP" "$ROOT/extracted" "$M" $MOTIONS 2>/dev/null | tail -1
"$MLP" "$ROOT/extracted" "$M" $MOTIONS >/dev/null 2>&1 \
  || { echo "  FAIL a ported move's motion entry does not match its source's"; rc=1; }
return $rc
}

gate_def gate_3j "3j. the hand-written specials use their source fighter's own param values"
gate_3j() {
local rc=0
# The generated ACMDs are copied verbatim and step 3i proves it. Alolan Whip, Revenge, the cargo
# carry and the up-B are hand-written, and every number in them was read out of the source's vl.prc
# once and typed in. This checks they still match, and -- the part that had no answer at all --
# that every param those objects contain is either used or explicitly declined.
bash "$ROOT/tools/cached.sh" srcparams bash "$ROOT/tools/check-srcparams.sh" || rc=1
return $rc
}

gate_def gate_3i "3i. every ported move carries its source's damage and knockback"
gate_3i() {
local rc=0
# check-hitboxes.sh measures WHERE a hitbox is and HOW BIG. This checks everything else the macro
# carries -- damage, angle, KBG, FKB, BKB, hitlag, SDI, collision attribute, sound level -- plus
# ATTACK_ABS, which holds the entire damage and knockback of the four DK cargo throws and has no
# geometry for the trace to see. "Exact frame data, damage, and range" is the project's own
# statement of intent, and only `range` had a gate.
bash "$ROOT/tools/cached.sh" acmdvalues bash "$ROOT/tools/check-acmd-values.sh" || rc=1
return $rc
}

gate_def gate_3c "3c. ported hitboxes land where the source character's do"
gate_3c() {
local rc=0
bash "$ROOT/tools/cached.sh" hitboxes bash "$ROOT/tools/check-hitboxes.sh" || rc=1
return $rc
}

gate_def gate_4 "4. fighter_param.prcxml parses and carries Pikachu's attributes"
gate_4() {
local rc=0
# Parses the file the way ARCropolis does and asserts the values are present at Pikachu's index.
# The previous version of this check only looked for a UTF-16 byte order mark -- and the BOM was
# the bug: ARCropolis parses prcxml with prcx + xml-rs 0.8, both UTF-8 only, so the file was
# silently dropped and every attribute stayed vanilla on hardware. Encoding checks cannot catch
# that; parsing can.
"$ROOT/tools/prc-tool/target/release/prcxmlcheck.exe" \
  "$ROOT/pikachu-moveset/mod/fighter/common/param/fighter_param.prcxml" 7 \
  weight=135 walk_speed_max=1.575 dash_speed=2.42 run_speed_max=2.464 \
  jump_y=50.51 mini_jump_y=17.34 jump_aerial_y=50.51 jump_initial_y=27.7805 \
  air_speed_x_stable=1.332 air_speed_y_stable=1.63 dive_speed_y=3.097 air_accel_y=0.13 \
  landing_attack_air_frame_n=12 landing_attack_air_frame_f=10 landing_attack_air_frame_b=8 \
  landing_attack_air_frame_hi=9 landing_attack_air_frame_lw=14 || rc=1
return $rc
}

gate_def gate_4b "4b. fighter_param_motion.prcxml carries Fox's dodge windows"
gate_4b() {
local rc=0
"$ROOT/tools/prc-tool/target/release/prcxmlcheck.exe" \
  "$ROOT/pikachu-moveset/mod/fighter/common/param/fighter_param_motion.prcxml" 7 \
  --table fighter_param_motion_table \
  escape_air_cancel_frame=40 escape_air_stiff_frame=38 escape_air_slide_stiff_frame=63 \
  escape_air_slide_d_stiff_frame=54 escape_air_slide_u_stiff_frame=75 escape_attack_frame=19 \
  escape_b_cancel_frame=33 escape_b_hit_normal_frame=11 escape_f_cancel_frame=27 \
  escape_f_hit_normal_frame=9 escape_f_penalty_hit_normal_frame=5 escape_n_cancel_frame=24 \
  escape_n_hit_normal_frame=12 escape_n_penalty_hit_normal_frame=10 || rc=1
return $rc
}

gate_def gate_4d "4d. nothing hands a victim to engine code that needs an article we do not ship"
gate_4d() {
local rc=0
bash "$ROOT/tools/check-victim.sh" || rc=1
return $rc
}

gate_def gate_4f "4f. a status flag the status code reads still has an ACMD that sets it"
gate_4f() {
local rc=0
bash "$ROOT/tools/check-keepflags.sh" || rc=1
return $rc
}

gate_def gate_4g "4g. every game_ script a shipped motion names is actually installed"
gate_4g() {
local rc=0
bash "$ROOT/tools/check-scripts.sh" || rc=1
return $rc
}

gate_def gate_4h "4h. the three hand-maintained port lists agree"
gate_4h() {
local rc=0
bash "$ROOT/tools/check-lists.sh" || rc=1
return $rc
}

gate_def gate_4i "4i. every Init/Exec/Exit the source status has is registered"
gate_4i() {
local rc=0
bash "$ROOT/tools/check-statuslines.sh" || rc=1
return $rc
}

gate_def gate_4k "4k. the grafted weapons agree across the merge, the plugin and config.json"
gate_4k() {
local rc=0
bash "$ROOT/tools/check-weapons.sh" || rc=1
return $rc
}

gate_def gate_4l "4l. a cloned article's donor code does not read its donor's param entry"
gate_4l() {
local rc=0
bash "$ROOT/tools/check-article-donor-params.sh" || rc=1
return $rc
}

# ADR 0002, gated. A fighter-prefixed effect name compiles, deploys and silently does nothing on a
# card where that fighter is absent, so nothing else in this harness can see it -- and the ADR
# itself predicted that a future reader would try to "fix" the generic visuals by wiring up the real
# names. One did, unattended, on 2026-09-18. The check audits its own coverage; see its header.
gate_def gate_4o "4o. no effect request names a non-resident archive (ADR 0002)"
gate_4o() {
local rc=0
PY_BIN=""
for c in py python3 python; do command -v "$c" >/dev/null 2>&1 && { PY_BIN="$c"; break; }; done
if [ -n "$PY_BIN" ]; then
    "$PY_BIN" "$ROOT/tools/check-effects.py" || rc=1
else
    echo "  !! no python on PATH -- ADR 0002 is unchecked"; rc=1
fi
return $rc
}

gate_def gate_4j "4j. nothing this mod does reaches a fighter that is not Pikachu"
gate_4j() {
local rc=0
bash "$ROOT/tools/check-blastradius.sh" || rc=1
return $rc
}

# The sibling of 4j, one boundary in: seven of Pikachu's eight costumes are still vanilla Pikachu
# since the moveset became its own slot, and a raw `Agent::new` or an ungated `skyline::hook` hands
# them the moveset anyway -- silently, because it only shows on a colour nobody tests.
gate_def gate_4p "4p. nothing this mod does reaches a Pikachu that is not Rat of Doom"
gate_4p() {
local rc=0
bash "$ROOT/tools/check-slot.sh" || rc=1
return $rc
}

gate_def gate_4c "4c. hand-written code uses no other fighter's constants"
gate_4c() {
local rc=0
bash "$ROOT/tools/check-status.sh" || rc=1
return $rc
}

# 4c checks WHICH constants are named; this checks what they are WORTH. Both are needed: naming
# Pikachu's own constant is no help if the number behind it came from a wrong row in a
# third-party table.
gate_def gate_4m "4m. the constant values the plugin compiles in match the game's own"
gate_4m() {
local rc=0
bash "$ROOT/tools/check-luaconst.sh" || rc=1
return $rc
}

gate_def gate_4e "4e. every new status is fully registered and actually used"
gate_4e() {
local rc=0
bash "$ROOT/tools/check-statusgraph.sh" || rc=1
return $rc
}

# A SHIPPED ANIMATION MUST NOT DRIVE A BONE PIKACHU DOES NOT HAVE. The trim in anim-dropin.sh
# was bypassed for every source fighter whose scale equalled Pikachu's -- see the header of
# check-anim-tracks.sh -- and nothing asserted its output, so two animations shipped for months
# with a source fighter's whole hand in them.
gate_def gate_4n "4n. no shipped animation drives a bone the skeleton lacks"
gate_4n() {
local rc=0
bash "$ROOT/tools/check-anim-tracks.sh" || rc=1
return $rc
}

gate_def gate_5 "5. character select rename is present and correctly encoded"
gate_5() {
local rc=0
bash "$ROOT/tools/check-name.sh" || rc=1
return $rc
}

# THE DRIVER. Launch up to CHECK_JOBS gates, and replay in index order as results land. Launching
# and replaying share one loop on purpose: a harness that prints nothing for four minutes and then
# everything at once reads as hung, so the log starts appearing as soon as gate 1 is in, while the
# slow ones are still running behind it.
gate_n=${#GATE_FNS[@]}
# A `gate_def` naming a function that does not exist is a typo that would otherwise show up as a
# gate quietly producing nothing. Check every name before running any of them: a harness that
# silently skips a gate is worse than one that refuses to start. The arithmetic loop rather than
# $(seq), because a spawn here costs 50-70ms and this file is about not spending those.
for ((i = 0; i < gate_n; i++)); do
    declare -F "${GATE_FNS[$i]}" >/dev/null \
        || { echo "!! gate $((i + 1)) (${GATE_TITLES[$i]}) has no function ${GATE_FNS[$i]}"; exit 2; }
done

launched=0; replayed=0; running=0
while [ "$replayed" -lt "$gate_n" ]; do
    while [ "$running" -lt "$CHECK_JOBS" ] && [ "$launched" -lt "$gate_n" ]; do
        run_gate "$launched" &
        launched=$((launched + 1)); running=$((running + 1))
    done
    # `wait -n` returns as soon as ANY one of them finishes, which is what makes this a sliding
    # window rather than 34/12 batches -- the 148s weapons gate does not hold twelve slots idle.
    if [ "$running" -gt 0 ]; then
        wait -n 2>/dev/null || true
        running=$((running - 1))
    fi
    while [ "$replayed" -lt "$gate_n" ] && [ -f "$GATE_DIR/$replayed.done" ]; do
        replay_gate "$replayed"; replayed=$((replayed + 1))
    done
    # Nothing running, nothing left to launch, and the next gate still has no result: its subshell
    # died without finishing. Name it and move on, because the alternative is this loop spinning
    # forever on a file that is never going to appear.
    if [ "$running" -eq 0 ] && [ "$launched" -ge "$gate_n" ] && [ "$replayed" -lt "$gate_n" ]; then
        echo; echo "== ${GATE_TITLES[$replayed]} =="
        echo "  FAIL no result -- the subshell running ${GATE_FNS[$replayed]} died before finishing"
        rc=1; replayed=$((replayed + 1))
    fi
done
wait

# One last stamp. The per-gate numbers no longer sum to the total -- that is the entire point --
# so the total is measured rather than added up, and it carries the job limit because a log read a
# month from now has to be able to say whether it was a twelve-wide run or a serial one.
CHECK_TEND=$(date +%s%N)
printf '  [%5.1fs wall across %d gates, %d at a time]\n' \
    "$(( (CHECK_TEND - CHECK_T0) / 1000000 ))e-3" "$gate_n" "$CHECK_JOBS"
echo; [ $rc -eq 0 ] && echo "ALL CHECKS PASSED" || echo "CHECKS FAILED"
exit $rc
