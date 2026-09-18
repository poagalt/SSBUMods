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
WANT=""

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

# ==== PALUTENA'S STAFF RIDES THE VANILLA `Mouth`, BECAUSE A MESH MUST BIND TO A BONE PIKACHU
# ==== ALREADY SHIPS, AND HERS IS THE ONE WEAPON THAT CANNOT BE BAKED
#
# THE A/B THAT SETTLED THE RULE is in `merge-weapons.sh`'s bake block, and it is worth repeating
# because it holds the geometry fixed and varies only the bone. Her staff shipped twice in the
# same place by construction -- same parent `HaveR`, same `Stick` bind transform, same vertices --
# once on the vanilla `HaveL` (index 17) and once on an appended `Stick` (index 88):
#
#     vanilla `HaveL`    correct facing right, wrong facing left
#     appended `Stick`   wrong in BOTH directions
#
# And the playtest says of the appended version "sweeps, but in the wrong place", so the bone IS
# driven -- her 65-keyframe `Stick` track moves it -- and it is the MESH that renders elsewhere.
# Four of the five weapons are therefore baked onto the vanilla `HaveR`. Hers cannot be: `Stick`
# carries 65 keyframes of the sweep that IS the move, and baking freezes it.
#
# SO A VANILLA BONE IS REPURPOSED INTO `Stick`'s FRAME -- which is what `HaveL` used to do, and
# `HaveL` was the wrong choice for two reasons that are both avoided here.
#
# WHY `Mouth` AND NOT `HaveL`:
#   - `HaveL` is the LEFT-HAND ITEM SOCKET and the engine has semantics for it. Reparenting it
#     under `HaveR` coincided exactly with Corrin's Yato breaking -- he single-binds to `HaveR`,
#     so he was the weapon most exposed to a foreign child on it -- and undoing the reparent
#     fixed him. It also silently moved two `sys_smash_flash` effects the source ACMD anchors to
#     `havel` onto the wrong hand.
#   - `HaveL` is index 17 and `HaveR` is index 22, so it was a child evaluated BEFORE its parent.
#     Ten bones in every fighter in the game have that property (`Jaw`, `Tongue`, the `Uplip*`
#     set), so it is not fatal in general -- but it is an anomaly, it is not worth carrying, and
#     `Mouth` at index 37 does not have it.
#
# `Mouth` IS FREE, CHECKED RATHER THAN ASSUMED. Of vanilla Pikachu's 77 bones it is one of five
# that are leaves, carry no mesh weight and are never named by `model.nuhlpb`: `Hat`, `HaveL`,
# `HaveR`, `Mouth`, `Throw`. `HaveR` is where the other four weapons bind; `Throw` is the throw
# attachment and this plugin names it twelve times; `Hat` is index 10 and would reintroduce the
# out-of-order parent. `Mouth` is named ZERO times anywhere in the plugin, and no SHIPPED
# animation drives it -- the source fighters have no such bone, so no ported move has the track.
#
# The cost: vanilla animations DO drive `Mouth`, so during a non-ported move the bone goes
# somewhere meaningless under `HaveR`. Nothing renders on it -- no vanilla mesh is weighted to it
# and `src/sword/mod.rs` shows the staff only during Palutena's own motions -- so the cost is
# invisible, which is the same argument that made `HaveL` acceptable minus the two objections.
STICKSRC="$ROOT/work/fk/palutena_skel.json"
if [ -f "$STICKSRC" ]; then
    hidx=$(jq '[.bones|to_entries[]|select(.value.name=="HaveR")][0].key' "$W/out.json")
    before=$(jq -r '(.bones|to_entries) as $bs | $bs[] | select(.value.name=="Mouth")
                    | $bs[.value.parent_index].value.name' "$W/out.json")
    jq --argjson p "$hidx" --slurpfile s "$STICKSRC" '
      ( [ $s[0].bones[] | select(.name == "Stick") ][0].transform ) as $st
      | .bones = [ .bones[]
          | if .name == "Mouth" then (.parent_index = $p | .transform = $st) else . end ]
    ' "$W/out.json" > "$W/next.json" && mv "$W/next.json" "$W/out.json"
    after=$(jq -r '(.bones|to_entries) as $bs | $bs[] | select(.value.name=="Mouth")
                   | $bs[.value.parent_index].value.name' "$W/out.json")
    echo "  ~ Mouth reparented $before -> $after, bind transform taken from palutena Stick"
    added=$((added+1))
else
    echo "  !! no $STICKSRC -- Mouth NOT repurposed, Palutena's staff will ride an appended bone" >&2
fi
if [ "$added" -gt 0 ]; then
    "$T" "$W/out.json" "$SKEL" >/dev/null
    echo "add-bones: $added change(s), skeleton now $(jq '.bones|length' "$W/out.json") bones"
else
    echo "add-bones: nothing to do ($(jq '.bones|length' "$W/out.json") bones)"
fi
