#!/usr/bin/env bash
# Full offline parity harness. Run before every install.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
M="$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/c00/motion_list.bin"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
rc=0

echo "== 1. hitbox bones exist on Pikachu =="
bash "$ROOT/tools/check-bones.sh" || rc=1

echo; echo "== 2. every registered ACMD is reachable from a motion entry =="
"$ROOT/tools/motion-patch/target/release/checkacmd.exe" "$M" "$ROOT/pikachu-moveset/plugin/src" || rc=1

echo; echo "== 3. shipped binaries reparse =="
for f in "$M" "$ROOT"/pikachu-moveset/mod/fighter/pikachu/model/body/c00/model.*; do
  case "$f" in *.nutexb|*.adjb) continue ;; esac
  n=$(basename "$f")
  if [ "$n" = "motion_list.bin" ]; then
    if "$ROOT/tools/motion-patch/target/release/motion-patch.exe" --help >/dev/null 2>&1 || true; then :; fi
    "$ROOT/tools/motion-patch/target/release/valcheck.exe" "$f" >/dev/null 2>&1 \
      && echo "  ok  $n" || { echo "  FAIL $n"; rc=1; }
  else
    "$T" "$f" /tmp/_rt.json >/dev/null 2>&1 && echo "  ok  $n" || { echo "  FAIL $n"; rc=1; }
  fi
done

echo; echo "== 3b. vl.prc matches whether an article clone is installed =="
# `param_spikeball` in Pikachu's vl.prc is only correct while clone_weapon is actually enabled.
# With the clone disabled it is dead config that still takes Pikachu from 20 article params to 21,
# declaring one with no weapon behind it -- and Training Modpack's save state iterates article
# indices calling get_article and remove_exist_object_id, which is a plausible crash on the 21st.
# So the two must agree in both directions.
V="$ROOT/pikachu-moveset/mod/fighter/pikachu/param/vl.prc"
A="$ROOT/pikachu-moveset/plugin/src/articles/mod.rs"
clone_on=$(grep -cx 'pub const CLONE_ENABLED: bool = true;' "$A" || true)
if [ -f "$V" ]; then
  k=$("$ROOT/tools/prc-tool/target/release/vlkeys.exe" "$V" 2>/dev/null | grep -c '0x000f679aaf13' || true)
else
  k=0
fi
if [ "$clone_on" != "0" ] && [ "$k" != "1" ]; then
  echo "  FAIL clone_weapon is enabled but vl.prc lacks param_spikeball -- it will null-deref"; rc=1
elif [ "$clone_on" = "0" ] && [ "$k" != "0" ]; then
  echo "  FAIL clone_weapon is disabled but vl.prc still ships param_spikeball -- dead article param"; rc=1
elif [ "$clone_on" = "0" ]; then
  echo "  ok  clone disabled, no vl.prc override shipped (Pikachu keeps his vanilla 20 article params)"
else
  echo "  ok  clone enabled and param_spikeball present"
fi

echo; echo "== 3c. ported hitboxes land where the source character's do =="
bash "$ROOT/tools/check-hitboxes.sh" || rc=1

echo; echo "== 4. fighter_param.prcxml parses and carries Pikachu's attributes =="
# Parses the file the way ARCropolis does and asserts the values are present at Pikachu's index.
# The previous version of this check only looked for a UTF-16 byte order mark -- and the BOM was
# the bug: ARCropolis parses prcxml with prcx + xml-rs 0.8, both UTF-8 only, so the file was
# silently dropped and every attribute stayed vanilla on hardware. Encoding checks cannot catch
# that; parsing can.
"$ROOT/tools/prc-tool/target/release/prcxmlcheck.exe" \
  "$ROOT/pikachu-moveset/mod/fighter/common/param/fighter_param.prcxml" 7 \
  weight=135 walk_speed_max=1.575 dash_speed=2.42 run_speed_max=2.464 \
  jump_y=50.51 mini_jump_y=17.34 jump_aerial_y=50.51 jump_initial_y=27.7805 \
  air_speed_x_stable=1.332 air_speed_y_stable=1.63 dive_speed_y=3.097 air_accel_y=0.13 \n  landing_attack_air_frame_n=12 landing_attack_air_frame_f=10 landing_attack_air_frame_b=8 \n  landing_attack_air_frame_hi=9 landing_attack_air_frame_lw=14 || rc=1

echo; echo "== 4b. fighter_param_motion.prcxml carries Fox's dodge windows =="
"$ROOT/tools/prc-tool/target/release/prcxmlcheck.exe" \
  "$ROOT/pikachu-moveset/mod/fighter/common/param/fighter_param_motion.prcxml" 7 \
  --table fighter_param_motion_table \
  escape_air_cancel_frame=40 escape_air_stiff_frame=38 escape_air_slide_stiff_frame=63 \
  escape_air_slide_d_stiff_frame=54 escape_air_slide_u_stiff_frame=75 escape_attack_frame=19 \
  escape_b_cancel_frame=33 escape_b_hit_normal_frame=11 escape_f_cancel_frame=27 \
  escape_f_hit_normal_frame=9 escape_f_penalty_hit_normal_frame=5 escape_n_cancel_frame=24 \
  escape_n_hit_normal_frame=12 escape_n_penalty_hit_normal_frame=10 || rc=1

echo; echo "== 4c. hand-written code uses no other fighter's constants =="
bash "$ROOT/tools/check-status.sh" || rc=1

echo; echo "== 5. character select rename is present and correctly encoded =="
bash "$ROOT/tools/check-name.sh" || rc=1

echo; [ $rc -eq 0 ] && echo "ALL CHECKS PASSED" || echo "CHECKS FAILED"
exit $rc
