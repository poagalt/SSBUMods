#!/usr/bin/env bash
# Measures every dropped-in move's hitbox error against the source character, per frame, using
# what the mod actually ships: the scale-compensated animation, the grafted skeleton, and our own
# generated ACMD. See tools/reach/src/bin/hbtrace.rs.
#
# Scales are read from tools/scales.tsv, which tools/gen-scales.sh derives from the game's own
# fighter_param.prc. They used to live in this file's move table and two were wrong, which the
# trace could not detect because it applied the same wrong constant to both sides.
#
# "NOT MEASURED" or "no FK data" means a move is unverified -- not that it is fine.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
H=tools/reach/target/release/hbtrace.exe
D=vendor/SSBU-Dumped-Scripts/smashline
S=pikachu-moveset/plugin/src
SC=tools/scales.tsv
[ -f "$SC" ] || { echo "no $SC -- run tools/gen-scales.sh"; exit 1; }
vendor/ssbh_lib/ssbh_data_json.exe \
  pikachu-moveset/mod/fighter/pikachu/model/body/c00/model.nusktb \
  work/fk/pikachu_shipped_skel.json >/dev/null
mkdir -p work/trace
while read -r f acmd fn stem mod label; do
  [ -z "${f// }" ] && continue
  case "$f" in \#*) continue ;; esac
  scale=$(awk -F'\t' -v x="$f" '$1==x{print $3}' "$SC")
  if [ -z "$scale" ]; then printf "  %-22s -- no scale for %s in scales.tsv\n" "$label" "$f"; continue; fi
  if [ ! -f "work/fk/${f}_skel.json" ] || [ ! -f "work/fk/${f}_${stem}.json" ]; then
    printf "  %-22s -- no FK data (skeleton or animation not dumped)\n" "$label"; continue
  fi
  printf "  %-22s " "$label"
  $H "$D/lua2cpp_$f/$f/$acmd.txt" "game_$fn" "$f" "$stem" "$scale" \
     --ours-acmd "$S/$mod/mod.rs" --ours-fn "pikachu_game_$fn" ${1:-} 2>&1 | tail -1 | sed 's/^ *//'
done < "$ROOT/tools/moves.tsv"
