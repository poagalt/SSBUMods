#!/usr/bin/env bash
# Parity harness: every bone a ported hitbox anchors to must exist on Pikachu.
#
# Ground truth is Pikachu's OWN vanilla ACMD dumps rather than a strings dump of
# model.nusktb -- `top` does not appear in a strings dump but is used 287 times by
# Pikachu's own scripts, so the skeleton file alone gives false positives.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PIKA_ACMD="$ROOT/vendor/SSBU-Dumped-Scripts/smashline/lua2cpp_pikachu"
SKEL="$ROOT/extracted/fighter/pikachu/model/body/c00/model.nusktb"

# Hash40 names that are not bones: mesh objects and effect/trail textures.
NOT_BONES='^(chrom_sword|tex_item_[a-z0-9]+)$'

strings -n 2 "$SKEL" | tr 'A-Z' 'a-z' | sort -u > /tmp/_pika_skel.txt
grep -rhoE 'Hash40::new\("[a-z0-9_]+"\)' "$PIKA_ACMD" \
  | sed 's/Hash40::new("//; s/")//' | sort -u > /tmp/_pika_used.txt

grep -ohE 'Hash40::new\("[a-z0-9_]+"\)' "$ROOT"/pikachu-moveset/plugin/src/*/mod.rs \
  | sed 's/Hash40::new("//; s/")//' | sort -u \
  | grep -vE '^(collision_attr|attack_|sys_|tex_|common|invalid)' \
  | grep -vE "$NOT_BONES" > /tmp/_candidates.txt

fail=0
while read -r b; do
  grep -qx "$b" /tmp/_pika_skel.txt && continue
  grep -qx "$b" /tmp/_pika_used.txt && continue
  echo "FAIL: hitbox bone '$b' does not exist on Pikachu"
  fail=1
done < /tmp/_candidates.txt

if [ "$fail" -eq 0 ]; then echo "bone check: OK ($(wc -l < /tmp/_candidates.txt) bones verified)"; fi
exit $fail
