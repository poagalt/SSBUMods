#!/usr/bin/env bash
# Ships each source fighter's own animation in Pikachu's motion directory, so a ported move's
# verbatim bone-anchored hitboxes land exactly where the source's do. See ADR 0003.
#
# SSBU names an animation the same on every fighter -- motion `attack_s3_s` is always
# `c01attacks3s.nuanmb` -- so this is a same-path replacement. No motion_list or config.json
# change is needed, and `tools/motion-patch/target/release/dropin.exe` pre-flights the flags.
#
# A move listed here MUST have its ACMD ported with the source's original bone anchors intact.
# Re-anchoring a hitbox to `top` on top of a dropped-in animation would double-compensate.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/extracted/fighter"
DST="$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/c00"
mkdir -p "$DST"

# source fighter | animation file | move it drives
MOVES="
chrom     | c00attack11.nuanmb    | jab (Chrom)
elight    | c00attackdash.nuanmb  | dash attack (Mythra)
elight    | c01attacks3s.nuanmb   | f-tilt (Mythra)
falco     | c02attackhi3.nuanmb   | u-tilt (Falco)
littlemac | c03attacks4s.nuanmb   | f-smash (Little Mac) -- carries his 21.45u lunge in Trans
fox       | c04attackhi4.nuanmb   | u-smash (Fox)
palutena  | c05attackairn.nuanmb  | n-air (Palutena)
demon     | c05attackairf.nuanmb  | f-air (Kazuya)
cloud     | c05attackairb.nuanmb  | b-air (Cloud)
kamui     | c05attackairhi.nuanmb | u-air (Corrin)
krool     | c05attackairlw.nuanmb | d-air (K. Rool)
"

n=0; miss=0
while IFS='|' read -r fighter anim label; do
    fighter="${fighter// /}"; anim="${anim// /}"; label="$(echo "$label" | sed 's/^ *//;s/ *$//')"
    [ -z "$fighter" ] && continue
    s="$SRC/$fighter/motion/body/c00/$anim"
    if [ ! -f "$s" ]; then
        echo "  MISSING  $fighter/$anim -- $label" >&2
        miss=$((miss+1)); continue
    fi
    cp "$s" "$DST/$anim"
    printf "  %-22s <- %-10s %s\n" "$anim" "$fighter" "$label"
    n=$((n+1))
done <<< "$MOVES"

echo "anim-dropin: $n animations shipped, $miss missing"
[ "$miss" -eq 0 ] || exit 1
