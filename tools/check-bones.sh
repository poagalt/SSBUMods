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

strings -n 2 "$SKEL" | tr 'A-Z' 'a-z' | sort -u > /tmp/_pika_skel.txt
grep -rhoE 'Hash40::new\("[a-z0-9_]+"\)' "$PIKA_ACMD" \
  | sed 's/Hash40::new("//; s/")//' | sort -u > /tmp/_pika_used.txt

fail=0
grep -ohE 'Hash40::new\("[a-z0-9_]+"\)' "$ROOT"/pikachu-moveset/plugin/src/*/mod.rs \
  | sed 's/Hash40::new("//; s/")//' | sort -u \
  | grep -vE '^(collision_attr|attack_|sys_|tex_|common|invalid)' \
  | while read -r b; do
      if grep -qx "$b" /tmp/_pika_skel.txt || grep -qx "$b" /tmp/_pika_used.txt; then
        continue
      fi
      echo "FAIL: hitbox bone '$b' does not exist on Pikachu"
      fail=1
    done
if [ "$fail" -eq 0 ]; then echo "bone check: OK"; fi
