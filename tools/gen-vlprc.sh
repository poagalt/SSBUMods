#!/usr/bin/env bash
# Builds the vl.prc the mod ships for Pikachu, from his own vanilla one.
#
# WHY HE NEEDS A MODIFIED ONE AT ALL. Alolan Whip's victim side is common engine code -- the seven
# FIGHTER_STATUS_KIND_SWING_GAOGAEN_* statuses, which is exactly why the move is portable at all --
# and that code reads its numbers off the GRABBER's `param_special_s`:
#
#   special_s_through_target_speed           11.2   how fast the victim is whipped at the ropes
#   special_s_through_target_return_speed     2.8   how fast they rebound
#   special_s_through_target_cancel_frame      30   how long the outbound leg lasts
#   special_s_through_target_speed_no_rope    1.6   the same throw with no ropes to bounce off
#
# Pikachu ALREADY HAS a `param_special_s`. It is Skull Bash's -- charge rates, flick thresholds --
# and it contains none of those keys, so every lookup the victim's status makes returns 0 and the
# whipped opponent goes nowhere. The object has to be REPLACED with Incineroar's, not merged
# alongside it, which is what vlmerge's --replace is for.
#
# WHAT IS DELIBERATELY NOT COPIED
#   param_special_lw   Revenge is implemented entirely plugin-side and reads nothing from vl.prc,
#                      so overwriting Thunder's params would buy nothing and risk something.
#   param_special_hi   same: the G&W up-B port hardcodes its two multipliers from his own status
#                      code, and Quick Attack's params are not consulted by anything left running.
#   param_spikeball    an ARTICLE param, and article params must track articles/mod.rs
#                      CLONE_ENABLED exactly -- see tools/check.sh step 3b. Adding one with no
#                      weapon behind it takes Pikachu from 20 article params to 21, and Training
#                      Modpack's save state walks article indices calling get_article and
#                      remove_exist_object_id.
#
# Regenerate with:  bash tools/gen-vlprc.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VAN="$ROOT/extracted/fighter/pikachu/param/vl.prc"
OUT="$ROOT/pikachu-moveset/mod/fighter/pikachu/param/vl.prc"
VLMERGE="$ROOT/tools/prc-tool/target/release/vlmerge.exe"

[ -f "$VAN" ] || { echo "no extracted vanilla vl.prc at $VAN" >&2; exit 1; }

mkdir -p "$(dirname "$OUT")"
# Always rebuild from the VANILLA file, never from the previous output. Rerunning against our own
# product is how topscale ended up double-scaling every offset.
"$VLMERGE" --replace \
  "$VAN" "$ROOT/extracted/fighter/gaogaen/param/vl.prc" param_special_s "$OUT" | sed 's/^/  /'
