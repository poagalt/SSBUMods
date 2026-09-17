#!/usr/bin/env bash
# Moves Palutena's `Stick` animation track onto the vanilla `HaveL`, in every shipped animation
# that has one.
#
# WHY. A grafted bone is not reliably driven at runtime. `telemetry::mirror_probe` proves it with
# two controls in the same log line: `handr`, a vanilla bone, reads IDENTICALLY in both facings on
# every frame -- so the pose is the same and the measurement is sound -- while `stick` and `sword`
# differ between facings in all three components INCLUDING Y, which no facing can change. And
# `sword2`, a bone the animation drives and this mod never grafts, comes back as the exact negation
# of `haver_from_top`, i.e. the fighter origin: a missing bone returns the origin rather than
# failing, and appended bones read like memory past the end of an array sized for Pikachu's vanilla
# bone count.
#
# Four of the five weapons stopped caring when `merge-weapons.sh` began baking them onto `HaveR`.
# Palutena's staff cannot be baked -- the probe shows her HAND nearly static from frame 6 while the
# staff traces a full circle, so the sweep is entirely this track -- so the track moves to a bone
# the runtime owns.
#
# `tools/add-bones.sh` reparents `HaveL` under `HaveR` and gives it Stick's bind transform, so
# `HaveL` occupies Stick's frame EXACTLY. That makes this a pure rename: the track's values are a
# local transform relative to `HaveR`, and after the reparent that is precisely what `HaveL`'s
# local transform means. Nothing is recomputed, so nothing can be recomputed wrongly.
#
# It also makes the hitbox side a pure rename, which is what `tools/bone-remap.sed` does. That file
# was emptied for being lossy -- it mapped `stick` to `haver` and discarded Stick's [0,-2.2,0]
# offset -- and mapping to a bone that HAS that offset discards nothing.
#
# WHICH ANIMATIONS: derived, not listed. Every shipped `.nuanmb` with a `Stick` node gets it --
# Palutena's n-air and her three grabs today, and anything ported later for free.
#
# AN EXISTING `HaveL` NODE IS REPLACED, NOT DUPLICATED. The nair already carries a one-keyframe
# `HaveL` track, and leaving it in place beside the renamed one would put two nodes of the same
# name in the file, where whichever the engine reads second wins.
#
# Runs from build.sh after anim-dropin.sh (which regenerates c00) and BEFORE mirror-costumes.sh
# (which copies c00 to the other seven). Idempotent: an animation with no `Stick` node is skipped.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
DST="$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/c00"
W="$ROOT/work/sticktrack"
CR=$(printf '\r')

[ -d "$DST" ] || { echo "  no c00 motion directory -- run tools/anim-dropin.sh first" >&2; exit 1; }
rm -rf "$W"; mkdir -p "$W"

renamed=0
for a in "$DST"/*.nuanmb; do
    [ -f "$a" ] || continue
    b=$(basename "$a" .nuanmb)
    "$T" "$a" "$W/$b.json" >/dev/null 2>&1 || continue
    n=$(jq -r '[.. | objects | select(.name? == "Stick")] | length' "$W/$b.json" 2>/dev/null | tr -d "$CR")
    [ "${n:-0}" = "0" ] && continue
    k=$(jq -r '[.. | objects | select(.name? == "Stick") | .tracks[]?.values.Transform | length] | add // 0' \
        "$W/$b.json" 2>/dev/null | tr -d "$CR")
    jq '
      .groups = [ .groups[]
        | .nodes = ( [ .nodes[]? | select(.name != "HaveL") ]
                     | map(if .name == "Stick" then .name = "HaveL" else . end) ) ]
    ' "$W/$b.json" > "$W/$b.out.json" || { echo "  FAILED to rewrite $b" >&2; exit 1; }
    # Refuse to ship a file that lost the track instead of moving it.
    got=$(jq -r '[.. | objects | select(.name? == "HaveL") | .tracks[]?.values.Transform | length] | add // 0' \
          "$W/$b.out.json" 2>/dev/null | tr -d "$CR")
    if [ "${got:-0}" != "${k:-0}" ]; then
        echo "  FAILED $b: Stick had ${k} keyframe(s), HaveL came out with ${got}" >&2; exit 1
    fi
    "$T" "$W/$b.out.json" "$a" >/dev/null 2>&1 || { echo "  FAILED to write $b" >&2; exit 1; }
    # AND THE DUMP hbtrace READS, or the parity harness measures the wrong thing.
    #
    # `anim-dropin.sh` writes `work/fk/shipped_<stem>.json` beside each `.nuanmb`, and
    # `hbtrace` prefers it for the OURS side precisely so the measurement reflects what ships
    # rather than the pristine source. This step runs AFTER anim-dropin, so rewriting only the
    # `.nuanmb` left that dump still naming `Stick` -- and the harness then looked up `havel` in
    # an animation that does not drive it, fell back to the bind pose, and reported
    # `nair_palutena is 14.03 world units off`. A real-looking number from a stale input.
    if [ -f "$ROOT/work/fk/shipped_$b.json" ]; then
        cp "$W/$b.out.json" "$ROOT/work/fk/shipped_$b.json"
    fi
    echo "  ~ $b: Stick -> HaveL (${k} keyframes)"
    renamed=$((renamed+1))
done
echo "  stick track: $renamed animation(s) repointed onto the vanilla HaveL"
