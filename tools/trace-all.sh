#!/usr/bin/env bash
# Measures every dropped-in move's hitbox error against the source character, per frame, using
# what the mod actually ships: the scale-compensated animation, the grafted skeleton, and our own
# generated ACMD. See tools/reach/src/bin/hbtrace.rs.
#
# "NOT MEASURED" means a bone is missing and the move is unverified -- not that it is fine.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
H=tools/reach/target/release/hbtrace.exe
D=vendor/SSBU-Dumped-Scripts/smashline
S=pikachu-moveset/plugin/src
vendor/ssbh_lib/ssbh_data_json.exe \
  pikachu-moveset/mod/fighter/pikachu/model/body/c00/model.nusktb \
  work/fk/pikachu_shipped_skel.json >/dev/null
mkdir -p work/trace
worst=0
while read -r f acmd fn stem scale mod label; do
  [ -z "${f// }" ] && continue
  case "$f" in \#*) continue ;; esac
  if [ ! -f "work/fk/${f}_skel.json" ]; then
    printf "  %-22s -- no FK skeleton extracted yet\n" "$label"; continue
  fi
  printf "  %-22s " "$label"
  $H "$D/lua2cpp_$f/$f/$acmd.txt" "game_$fn" "$f" "$stem" "$scale" \
     --ours-acmd "$S/$mod/mod.rs" --ours-fn "pikachu_game_$fn" ${1:-} 2>&1 | tail -1 | sed 's/^ *//'
done < "$ROOT/tools/moves.tsv"
