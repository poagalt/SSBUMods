#!/usr/bin/env bash
# Measures every dropped-in move's hitbox error against the source character, per frame.
# See tools/reach/src/bin/hbtrace.rs. Non-zero deviation is the mod's real inaccuracy in
# world units; "NOT MEASURED" means a bone is missing and the move is unverified, not fine.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
H=tools/reach/target/release/hbtrace.exe
D=vendor/SSBU-Dumped-Scripts/smashline
vendor/ssbh_lib/ssbh_data_json.exe pikachu-moveset/mod/fighter/pikachu/model/body/c00/model.nusktb \
  work/fk/pikachu_shipped_skel.json >/dev/null
while read -r f acmd fn stem scale label; do
  [ -z "${f// }" ] && continue
  case "$f" in \#*) continue ;; esac
  [ -f "work/fk/${f}_skel.json" ] || { printf "  %-26s -- no FK skeleton extracted yet\n" "$label"; continue; }
  printf "  %-26s " "$label"
  $H "$D/lua2cpp_$f/$f/$acmd.txt" "game_$fn" "$f" "$stem" "$scale" ${1:-} 2>&1 | tail -1 | sed 's/^ *//'
done < "$ROOT/tools/moves.tsv"
