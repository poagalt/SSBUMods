#!/usr/bin/env bash
# Renders each traced move as a side view (z forward, y up), so the mod's hitboxes can be
# compared against the source character's by eye rather than by description.
#
# Three layers per panel:
#   REFERENCE -- where that character's hitbox actually is, per active frame (ground truth)
#   OURS      -- where the mod puts it, same frames
#   OLD       -- the retired reanchor capsules, when a *.old.tsv is present, to show what the
#                playtest reports of "a rod above pikachu" were describing
#
# usage: hbsvg.sh <out.svg> <move.json>...
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$1"; shift

W=380; H=300; COLS=2; PAD=44
n=0
: > /tmp/panels.tsv
for j in "$@"; do
    stem=$(basename "$j" .json)
    jq -r --arg m "$stem" '.hits[] | select(.ok) |
        [$m, .f, .id, .size, .ref[1], .ref[2], .ours[1], .ours[2], .size_ours] | @tsv' "$j" >> /tmp/panels.tsv
    n=$((n+1))
done

rows=$(( (n + COLS - 1) / COLS ))
SW=$(( COLS * W )); SH=$(( rows * H + 40 ))

{
cat <<SVG
<svg viewBox="0 0 $SW $SH" xmlns="http://www.w3.org/2000/svg" font-family="ui-sans-serif, system-ui, sans-serif">
<style>
  .ttl{font-size:13px;font-weight:600;fill:var(--text-primary,#111)}
  .sub{font-size:10px;fill:var(--text-secondary,#666)}
  .ax{stroke:var(--border-color,#ccc);stroke-width:1}
  .axl{font-size:9px;fill:var(--text-secondary,#888)}
  .refc{fill:none;stroke:#2f8f4e;stroke-width:1.6;opacity:.75}
  .ourc{fill:none;stroke:#2563eb;stroke-width:1.2;stroke-dasharray:3 2;opacity:.85}
  .oldc{stroke:#dc2626;stroke-width:2.5;opacity:.65;fill:none}
  .pika{fill:#f5c542;stroke:#8a6b00;stroke-width:1}
  .lg{font-size:10px}
</style>
<text x="12" y="20" class="ttl">Hitbox position vs the source character - side view, world units, Pikachu at origin</text>
<text x="12" y="34" class="sub">green = the source character's real hitbox (ground truth) | blue dashed = this mod | red = the retired reanchor capsules</text>
SVG

awk -F'\t' -v W=$W -v H=$H -v COLS=$COLS -v PAD=$PAD -v ROOT="$ROOT" '
BEGIN{ order_n=0 }
{
  m=$1; f=$2; id=$3; sz=$4; ry=$5; rz=$6; oy=$7; oz=$8;
  if (!(m in seen)) { seen[m]=1; order[order_n++]=m }
  k=m; cnt[k]++;
  RY[k,cnt[k]]=ry; RZ[k,cnt[k]]=rz; OY[k,cnt[k]]=oy; OZ[k,cnt[k]]=oz; SZ[k,cnt[k]]=sz; SO[k,cnt[k]]=$9;
  if (ry+sz>maxy[k]||!(k in maxy)) maxy[k]=ry+sz;
}
END{
  # A single shared scale across panels keeps the moves visually comparable.
  lo=-26; hi=26; ylo=-8; yhi=30;
  for (i=0;i<order_n;i++){
    m=order[i]; col=i%COLS; row=int(i/COLS);
    ox=col*W; oy0=row*H+40;
    cx=ox+PAD; cy=oy0+H-30; pw=W-PAD-14; ph=H-52;
    sx=pw/(hi-lo); sy=ph/(yhi-ylo);
    X0=cx+(0-lo)*sx; Y0=cy-(0-ylo)*sy;
    printf "<text x=\"%.0f\" y=\"%.0f\" class=\"ttl\">%s</text>\n", ox+PAD, oy0+2, m;
    # axes
    printf "<line x1=\"%.1f\" y1=\"%.1f\" x2=\"%.1f\" y2=\"%.1f\" class=\"ax\"/>\n", cx, Y0, cx+pw, Y0;
    printf "<line x1=\"%.1f\" y1=\"%.1f\" x2=\"%.1f\" y2=\"%.1f\" class=\"ax\"/>\n", X0, oy0+8, X0, cy;
    for (t=-20;t<=20;t+=10){ px=X0+t*sx; printf "<text x=\"%.1f\" y=\"%.1f\" class=\"axl\" text-anchor=\"middle\">%d</text>\n", px, Y0+11, t }
    for (t=0;t<=25;t+=10){ py=Y0-t*sy; printf "<text x=\"%.1f\" y=\"%.1f\" class=\"axl\" text-anchor=\"end\">%d</text>\n", X0-24*sx*0+cx-4, py+3, t }
    # Pikachu, to scale: 6.9 world units tall
    printf "<rect x=\"%.1f\" y=\"%.1f\" width=\"%.1f\" height=\"%.1f\" rx=\"3\" class=\"pika\"/>\n", X0-2.4*sx, Y0-6.9*sy, 4.8*sx, 6.9*sy;
    for (c=1;c<=cnt[m];c++){
      printf "<circle cx=\"%.1f\" cy=\"%.1f\" r=\"%.1f\" class=\"refc\"/>\n", X0+RZ[m,c]*sx, Y0-RY[m,c]*sy, SZ[m,c]*sx;
      # our radius, not the reference radius: the game multiplies hitbox size by the fighter
      # `scale` param, and Pikachu 0.96 vs e.g. Mythra 1.112 makes ours genuinely smaller.
      printf "<circle cx=\"%.1f\" cy=\"%.1f\" r=\"%.1f\" class=\"ourc\"/>\n", X0+OZ[m,c]*sx, Y0-OY[m,c]*sy, SO[m,c]*sx;
    }
    # retired reanchor capsules, if captured
    fn=ROOT "/work/trace/" m ".old.tsv";
    while ((getline line < fn) > 0){
      split(line, a, "\t");
      printf "<line x1=\"%.1f\" y1=\"%.1f\" x2=\"%.1f\" y2=\"%.1f\" class=\"oldc\"/>\n", \
        X0+a[3]*sx, Y0-a[2]*sy, X0+a[5]*sx, Y0-a[4]*sy;
    }
    close(fn);
    printf "<text x=\"%.0f\" y=\"%.0f\" class=\"sub\">%d hitbox-frames</text>\n", ox+PAD, cy+22, cnt[m];
  }
}' /tmp/panels.tsv

echo "</svg>"
} > "$OUT"
echo "wrote $OUT ($(wc -c < "$OUT") bytes, $n panels)"
