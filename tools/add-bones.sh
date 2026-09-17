#!/usr/bin/env bash
# Adds the weapon bones a ported ACMD anchors hitboxes to but Pikachu's skeleton lacks.
#
# Chrom's jab rides `Sword1` and Palutena's n-air rides `Stick`. Both are children of `HaveR`,
# the universal item-hold bone, so they graft onto Pikachu unchanged: their bind transform is
# already expressed relative to HaveR.
#
# This replaces the old `bone-remap.sed` workaround, which rewrote both to `haver`. That was
# lossy in two ways that the drop-in architecture (ADR 0003) makes unnecessary:
#   - `Sword1` sits at HaveR's origin but carries a large bind ROTATION. Chrom's jab hitboxes are
#     offset (0,0,1.7) and (0,0,8.4) along the sword's own axis, so dropping the rotation aimed an
#     8.4-unit offset in the wrong direction entirely.
#   - `Stick` sits at [0,-2.2,0] from HaveR, so the remap also lost that offset. Palutena's n-air
#     hitboxes are at (0,+/-5.2,0) along the staff -- the observed "one missing hit".
# Both are animated in their source animation, so once the bone exists the dropped-in animation
# drives it and the hitbox is exact.
#
# Idempotent: a bone already present is left alone. Operates on the shipped model.nusktb, so run
# it after tools/merge-sword.sh.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
SKEL="$ROOT/pikachu-moveset/mod/fighter/pikachu/model/body/c00/model.nusktb"
W="$ROOT/work/bones"
mkdir -p "$W"

[ -f "$SKEL" ] || { echo "no shipped nusktb at $SKEL" >&2; exit 1; }
"$T" "$SKEL" "$W/pika.json" >/dev/null

# bone | source skeleton json | why
#
# EMPTY, BECAUSE NO HITBOX RIDES A GRAFTED BONE ANY MORE. Chrom's jab is rebased onto `haver` by
# `tools/reach/src/bin/rebase.rs` -- exact for him, measured 0.00, because `Sword1` does not move
# relative to the hand across his three live frames -- and Palutena's n-air rides the vanilla
# `HaveL` that the block at the bottom of this file reparents into `Stick`'s frame.
#
# The playtest is what decided it, and it separates the two cases cleanly: "the hitbox on cloud
# back air is fine both directions (unlike the other swords)". Cloud's ride `haver` and `top`, both
# vanilla; Chrom's rode `sword1` and Palutena's `stick`, both grafted. A bone Pikachu owns behaves
# and an appended one does not.
#
# Leaving the list here rather than deleting the mechanism: a future port whose hitboxes ride a
# bone that genuinely cannot be rebased -- one that moves relative to the hand across its live
# window, the way Palutena's staff does -- would need either a row here or the `realise` treatment,
# and the second is now the better answer.
WANT="Stick | $ROOT/work/fk/palutena_skel.json | palutena's staff bone, exactly where her own skeleton puts it"

cp "$W/pika.json" "$W/out.json"
added=0
while IFS='|' read -r bone src why; do
    bone="${bone// }"; src="${src// }"; why="$(echo "$why" | sed 's/^ *//;s/ *$//')"
    [ -z "$bone" ] && continue

    if jq -e --arg b "$bone" '.bones[] | select(.name == $b)' "$W/out.json" >/dev/null 2>&1; then
        echo "  $bone already present, skipped"
        continue
    fi
    # The source's parent MUST be HaveR for a verbatim graft; refuse rather than misplace it.
    pname=$(jq -r --arg b "$bone" '(.bones|to_entries) as $bs
        | $bs[] | select(.value.name==$b) | $bs[.value.parent_index].value.name' "$src")
    if [ "$pname" != "HaveR" ]; then
        echo "  REFUSING $bone: parent in source is '$pname', not HaveR -- bind transform would be wrong" >&2
        exit 1
    fi
    hidx=$(jq '[.bones|to_entries[]|select(.value.name=="HaveR")][0].key' "$W/out.json")
    jq --argjson p "$hidx" --arg b "$bone" --slurpfile s "$src" \
      '.bones += [ ($s[0].bones[] | select(.name == $b) | .parent_index = $p) ]' \
      "$W/out.json" > "$W/next.json" && mv "$W/next.json" "$W/out.json"
    echo "  + $bone  (parent HaveR = index $hidx) -- $why"
    added=$((added+1))
done <<< "$WANT"

# ==== `HaveL` IS BACK UNDER `HandL`, BECAUSE REPARENTING IT WAS THE ONE THING NO NATIVE SKELETON
# ==== DOES, AND PALUTENA'S STAFF NOW RIDES `Stick` EXACTLY WHERE SHE DOES
#
# What used to be here reparented Pikachu's vanilla `HaveL` under `HaveR`, gave it Palutena's
# `Stick` bind transform, renamed her animation's `Stick` track onto it, and bound the staff there.
# It is deleted, and the reasoning that produced it was wrong at the root.
#
# IT EXISTED BECAUSE OF AN INSTRUMENT ERROR. `telemetry::mirror_probe` reported that `sword`,
# `sword1` and `stick` "differ between facings in all three components including Y, which no facing
# can change", and that was read as proof that a grafted bone is not in the runtime transform
# chain -- hence the scheme to move the staff onto a bone Pikachu already owned. The probe cannot
# see grafted bones at all: in the latest capture `sword`, `sword1` and `fusionsword1` return
# BYTE-IDENTICAL positions to each other on all 262 samples, and they are three different bones
# with three different bind transforms (an arbitrary rotation, a 90-degree turn about Z, and the
# identity), so they cannot be in one place. A name the skeleton lacks returns the fighter origin
# rather than failing. Every grafted-bone reading this project has taken from that call is noise.
#
# AND THE VISUALS REFUTE IT DIRECTLY: Cloud's Buster Sword is single-bound to the GRAFTED
# `fusionsword1` and the playtest calls it correct. A grafted bone is in the render chain.
#
# WHAT THE GAME ACTUALLY DOES, which is the whole reason this is a deletion and not a rewrite.
# Palutena's staff is a mesh in her BODY model single-bound to `Stick`, and `Stick` is a real bone
# in her skeleton parented to `HaveR` with transform [[1,0,0],[0,1,0],[0,0,1]] + (0,-2.2,0). So the
# native structure is `HandR -> HaveR -> Stick`, and `WANT` above now grafts exactly that. Her
# `Stick` animation track needs no rename, her hitboxes need no remap, and `bone-remap.sed` goes
# back to a no-op -- all three of those existed only to serve the repurpose.
#
# WHY THE REPARENT WAS ACTIVELY HARMFUL, and this is measured. Every native skeleton checked --
# Corrin's, Palutena's, and vanilla Pikachu's -- parents `HaveL` to `HandL`. Ours was the only one
# that made it a child of `HaveR`, and the probe finds BOTH `HaveR` and `HaveL` in the wrong place
# facing left while `HandR` is correct: |handr - haver| is 1.030 on every frame facing right and
# 9.14 .. 13.31 facing left, on frames where |handr - origin| agrees between facings to 0.002, so
# the pose is provably identical. Corrin's Yato is single-bound to `HaveR` in his own body model
# and works in the retail game, so `HaveR` is a legitimate mesh parent and the bone is not at
# fault; the one difference between his skeleton and ours was the child we hung on it.
#
# NOT the bone ORDER, which was the first theory and is dead. `HaveL(17)` under `HaveR(22)` is a
# child evaluated before its parent -- but so are ten bones in every fighter in the game
# (`UplipL1`, `Jaw`, `Tongue` and the rest all have parents 40-odd indices later, in Pikachu,
# Corrin and Palutena alike). The engine handles that fine. Out-of-order parents are normal; a
# left-hand item socket hanging off the right hand is not.

if [ "$added" -gt 0 ]; then
    "$T" "$W/out.json" "$SKEL" >/dev/null
    echo "add-bones: $added change(s), skeleton now $(jq '.bones|length' "$W/out.json") bones"
else
    echo "add-bones: nothing to do ($(jq '.bones|length' "$W/out.json") bones)"
fi
