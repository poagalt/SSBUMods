#!/usr/bin/env bash
# Nothing this mod does reaches a Pikachu that is not Rat of Doom.
#
# THE SIBLING OF GATE 4j, one level in. `check-blastradius.sh` asserts the mod never reaches a
# fighter that is not Pikachu. Since the moveset became its own character slot there is a second
# boundary inside that one: seven of Pikachu's eight costumes are still VANILLA Pikachu, and every
# way this mod has of touching a fighter has to respect it.
#
# THE FOUR WAYS IT CAN LEAK, which is what the four checks below are:
#
#   1. A raw `Agent::new("pikachu")`. smashline installs acmd, status, line and state callbacks
#      under whatever costume list the Agent carries, and `Agent::default()`'s is EMPTY, which means
#      every costume. `crate::charaslot::agent()` is the only constructor that sets one, so a raw
#      call is an ungated registration -- and it compiles, deploys and looks fine until someone
#      picks c03.
#   2. A `skyline::hook`. smashline's costume filter cannot reach one; those are installed globally
#      and see every object in the fight. The two that act on fighter state -- `src/params`' four
#      WorkModule replacements and `src/articles/neutralize.rs`' WEAPON_INITIALIZE -- have to test
#      the costume themselves, which they do through `charaslot::is_rat_costume`.
#   3. A file under a costume directory that is not the rat's. ARCropolis does not care which build
#      step put a file there or whether anything still declares it: if the path matches an arc path,
#      it is served. An animation left in `motion/body/c05` is Rat of Doom's animation playing on
#      vanilla Pikachu, from a file nothing mentions.
#   4. A `new-dir-files` key for a costume that is not the rat's. That is the declaration half of 3,
#      and it can disagree with the tree in either direction.
#
# WHAT THIS DELIBERATELY DOES NOT CHECK: `fighter/common/param/fighter_param.prcxml`. That patches
# PIKACHU'S ROW of the game's own table, which is per-fighter and has no costume dimension at all,
# so the leak is real and unfixable rather than a mistake to catch -- see the note at the top of
# plugin/src/charaslot/mod.rs. Flagging it here would be a gate that can only ever fail.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
PLUGIN="$ROOT/pikachu-moveset/plugin"
MOD="$ROOT/pikachu-moveset/mod"
CHARASLOT="$PLUGIN/src/charaslot/mod.rs"
rc=0

# THE SLOT NUMBER, read from the declaration. Same parse as mirror-costumes.sh, on purpose: if the
# two ever read it differently, the one that is wrong is the one shipping assets to a dead colour.
slot=$(sed -n 's/^pub const RAT_COSTUME: usize = \([0-9]\+\);.*/\1/p' "$CHARASLOT")
if ! [[ "$slot" =~ ^[0-9]+$ ]] || [ "$slot" -gt 7 ]; then
    echo "  FAIL could not read a 0-7 RAT_COSTUME out of plugin/src/charaslot/mod.rs (got '${slot}')"
    exit 1
fi
rat=$(printf "c%02d" "$slot")
echo "  Rat of Doom is Pikachu $rat; the other seven costumes must be vanilla"

# ---- 1. every Pikachu Agent is built through the gate ----------------------------------------
# `charaslot/mod.rs` itself is excluded because it is where the gated constructor calls
# `Agent::new` -- that one call is the gate, not a hole in it.
raw=$(grep -rn 'Agent::new("pikachu")' "$PLUGIN/src" --include=*.rs | grep -v '/charaslot/mod.rs:' || true)
if [ -n "$raw" ]; then
    echo "  FAIL ungated Pikachu agent(s) -- these install on ALL eight costumes:"
    printf '%s\n' "$raw" | sed "s|^$PLUGIN/||" | sed 's/^/         /'
    rc=1
else
    n=$(grep -rc 'crate::charaslot::agent("pikachu")' "$PLUGIN/src" --include=*.rs | awk -F: '{s+=$2} END{print s+0}')
    echo "  ok  all $n Pikachu agent registration(s) are built through charaslot::agent()"
fi
# The generator emits the call too, and a regenerated module overwrites the hand edit every build.
if grep -q 'Agent::new(\\"pikachu\\")' "$ROOT/tools/gen-moves.sh"; then
    echo "  FAIL tools/gen-moves.sh still emits a raw Agent::new -- the generated modules will"
    echo "       lose the gate on the next build"
    rc=1
else
    echo "  ok  tools/gen-moves.sh emits the gated constructor"
fi

# ---- 1b. the module is actually called ----------------------------------------------------------
# THE FAILURE THIS EXISTS FOR ALREADY HAPPENED. `mod charaslot;` was added with the CSK API and
# `charaslot::install()` was never written into `main`, so the roster row was committed, built,
# gated, deployed and never registered -- a dead module that every other check was happy with,
# because nothing else in the harness reads lib.rs for whether an installer is reached. Found by
# noticing its diag line had never once appeared in a capture.
if grep -qE '^\s*charaslot::install\(\);' "$PLUGIN/src/lib.rs"; then
    echo "  ok  lib.rs calls charaslot::install()"
else
    echo "  FAIL plugin/src/lib.rs declares \`mod charaslot\` but never calls charaslot::install()"
    echo "       -- the roster row is compiled and never registered"
    rc=1
fi

# ---- 2. the global hooks test the costume themselves ------------------------------------------
for f in src/params/mod.rs src/articles/neutralize.rs; do
    if grep -q "charaslot::is_rat_costume" "$PLUGIN/$f"; then
        echo "  ok  $f tests the costume (smashline's filter does not reach a skyline::hook)"
    else
        echo "  FAIL $f installs a global hook and never tests charaslot::is_rat_costume"
        rc=1
    fi
done

# ---- 3. no shipped file under another costume's directory --------------------------------------
stray=$(find "$MOD/fighter" -type d -regextype posix-extended -regex ".*/c[0-9]{2}$" 2>/dev/null \
        | sed -E 's#.*/(c[0-9]{2})$#\1#;' | sort -u | grep -v "^$rat$" || true)
if [ -n "$stray" ]; then
    echo "  FAIL the tree ships costume directory/ies that are not the rat's:"
    for c in $stray; do
        d=$(find "$MOD/fighter" -type d -name "$c" | head -3)
        cnt=$(find "$MOD/fighter" -type d -name "$c" -exec find {} -type f \; | wc -l)
        printf '         %s  (%s file(s)) %s\n' "$c" "$cnt" "$(printf '%s ' $d | sed "s|$MOD/||g")"
    done
    echo "         run tools/mirror-costumes.sh -- it retires these"
    rc=1
else
    kept=$(find "$MOD/fighter" -type d -name "$rat" -exec find {} -type f \; | wc -l)
    echo "  ok  the tree ships $rat alone ($kept file(s)); no vanilla costume directory survives"
fi

# ---- 4. config.json declares only the rat's costume ---------------------------------------------
if [ -f "$MOD/config.json" ]; then
    bad=$(grep -oE '"fighter/pikachu/c[0-9]{2}"' "$MOD/config.json" | tr -d '"' | sed 's|.*/||' | sort -u | grep -v "^$rat$" || true)
    if [ -n "$bad" ]; then
        echo "  FAIL config.json has new-dir-files key(s) for a costume that is not the rat's:"
        printf '%s\n' "$bad" | sed 's/^/         fighter\/pikachu\//'
        echo "         run tools/gen-config.sh"
        rc=1
    else
        echo "  ok  config.json declares fighter/pikachu/$rat and no other costume"
    fi
else
    echo "  FAIL no $MOD/config.json"
    rc=1
fi

exit $rc
