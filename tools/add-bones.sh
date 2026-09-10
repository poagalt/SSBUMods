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
WANT="
Sword1 | $ROOT/work/fk/chrom_skel.json    | Chrom jab hitboxes ride the sword
Stick  | $ROOT/work/fk/palutena_skel.json | Palutena n-air hitboxes ride the staff
"

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

if [ "$added" -gt 0 ]; then
    "$T" "$W/out.json" "$SKEL" >/dev/null
    echo "add-bones: $added added, skeleton now $(jq '.bones|length' "$W/out.json") bones"
else
    echo "add-bones: nothing to do ($(jq '.bones|length' "$W/out.json") bones)"
fi
