#!/usr/bin/env bash
# Produces the SHIPPED tree: the authored mod, moved onto Rat of Doom's added costume slot.
#
# TWO TREES, AND WHY. `pikachu-moveset/mod` is the AUTHORING tree and it is written in the host's
# own coordinate system -- c00, because that is the costume every generator and every gate reads.
# `pikachu-moveset/ship` is what the card gets, with every path moved onto the added slot. The card
# and the authoring tree are therefore not the same shape, and that is deliberate.
#
# THE ALTERNATIVE WAS EIGHTEEN EDITS IN TWELVE TOOLS. `add-bones.sh`, `anim-dropin.sh`,
# `check-anim-tracks.sh`, `check-animsize.sh`, `check-bones.sh`, `check-scripts.sh`,
# `check-weapons.sh`, `cached.sh` and `build.sh` all name `pikachu-moveset/mod/fighter/pikachu/
# .../c00` directly, and each of them also names a DONOR's c00 (`extracted/fighter/littlemac/
# motion/body/c00`) which must not move. Renaming in place means getting that distinction right
# eighteen times, in shell, with no type checker -- and a miss is silent, because a gate that reads
# an empty directory reports zero problems. Relocating once, here, at the end, leaves all of them
# reading exactly what they read before.
#
# WHAT MOVES: `fighter/pikachu/**/cNN/` -> `**/c<rat>/`. Nothing else is rewritten, because nothing
# else in the tree is costume-addressed -- `ui/message/msg_name.xmsbt` and `info.toml` are
# fighter-wide and are copied through unchanged. `config.json` is NOT copied: `gen-config.sh` writes
# it into the shipped tree afterwards, from the shipped tree, so it can never describe the authoring
# tree's paths.
#
# WHY AN ADDED SLOT AT ALL -- see plugin/src/charaslot/mod.rs. Short version: taking one of the
# host's eight means that costume is reachable from his CSS row too, which the first hardware test
# reported as "both pikachu slots appear to load rat of doom". An added colour is outside his range,
# so his eight are untouched by construction.
#
# Idempotent: the shipped tree is rebuilt from scratch each run, so a file dropped from the
# authoring tree cannot survive here -- which is the same property `deploy.sh`'s retire list depends
# on, one level earlier.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MOD="$ROOT/pikachu-moveset/mod"
SHIP="$ROOT/pikachu-moveset/ship"
CHARASLOT="$ROOT/pikachu-moveset/plugin/src/charaslot/mod.rs"

# THE SLOT NUMBER, parsed from the declaration rather than restated. `RAT_COSTUME` in the plugin is
# the single source of truth: the plugin installs its agents under it, this ships assets for it, and
# gate 4p fails if the two disagree. 0-127 because an added slot is conventionally in the hundreds
# (Sans takes palutena c120-c127, Gogeta minmin c103-c110) and 127 is where a u8 colour index and
# ARCropolis's own slot handling stop being obviously safe.
if [ ! -f "$CHARASLOT" ]; then
    echo "  REFUSING: cannot find $CHARASLOT to read the slot number from" >&2
    exit 1
fi
slot=$(sed -n 's/^pub const RAT_COSTUME: usize = \([0-9]\+\);.*/\1/p' "$CHARASLOT")
if ! [[ "$slot" =~ ^[0-9]+$ ]] || [ "$slot" -gt 127 ]; then
    echo "  REFUSING: could not read a 0-127 RAT_COSTUME out of plugin/src/charaslot/mod.rs (got '${slot}')" >&2
    exit 1
fi
rat=$(printf "c%02d" "$slot")

if [ ! -d "$MOD/fighter/pikachu" ]; then
    echo "  REFUSING: $MOD has no fighter/pikachu -- run the earlier build steps first" >&2
    exit 1
fi

rm -rf "$SHIP"
mkdir -p "$SHIP"

moved=0
through=0
while IFS= read -r rel; do
    [ -z "$rel" ] && continue
    case "$rel" in
        config.json) continue ;;
    esac
    # Rewrite ONLY a costume component under fighter/pikachu, and only a whole path element --
    # `s|/c00/|/cNNN/|` on the raw string would also hit a filename, and 23 of the shipped
    # animations are named `c00attack11.nuanmb` and friends (that leading `c00` is SSBU's animation
    # GROUP naming, not a costume -- renaming one breaks the motion_list entry that names it).
    dst="$rel"
    case "$rel" in
        fighter/pikachu/*)
            dst=$(printf '%s' "$rel" | sed -E "s#(^fighter/pikachu/.*)/c[0-9]{2,3}/#\1/$rat/#")
            ;;
    esac
    mkdir -p "$SHIP/$(dirname "$dst")"
    cp "$MOD/$rel" "$SHIP/$dst"
    if [ "$dst" != "$rel" ]; then moved=$(( moved + 1 )); else through=$(( through + 1 )); fi
done < <(cd "$MOD" && find . -type f | sed 's|^\./||')

echo "  shipped tree at $rat: $moved relocated file(s), $through copied through"
