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
# THE CLONE DOES NOT REQUIRE param_spikeball, and this check used to insist that it did.
#
# The claim was that smashline null-dereferences without it. Reading vendor/sl_params.rs settles
# it: get_param_reimpl looks the key up in the filtered map and, on a miss, calls
# `(data.original_lookup_param.unwrap())(object, key)` -- it falls through to the fighter's own
# param holder. A missing article param means the spikeball runs on fall-through values, not a
# crash. And the crash it was blamed for was in load_fighter_effects, a different function that
# handles effect search indices and never touches params.
#
# So shipping the param object is an IMPROVEMENT (real values instead of fall-through), not a
# prerequisite -- which is what makes the clone testable without modifying vl.prc at all, and
# therefore testable as a single variable.
if [ "$clone_on" != "0" ] && [ "$k" != "1" ]; then
  echo "  note clone enabled without param_spikeball -- the spikeball will use fall-through params"
elif [ "$clone_on" = "0" ] && [ "$k" != "0" ]; then
  echo "  FAIL clone_weapon is disabled but vl.prc still ships param_spikeball -- dead article param"; rc=1
elif [ "$clone_on" = "0" ]; then
  echo "  ok  clone disabled, vl.prc ships no article param"
else
  echo "  ok  clone enabled and param_spikeball present"
fi

# THE OVERRIDE MUST NOT CHANGE THE ARTICLE COUNT, and it must actually be Incineroar's
# param_special_s rather than Pikachu's.
#
# The two are separate failures with the same symptom. Vanilla Pikachu has 20 top-level vl.prc
# entries; gen-vlprc.sh REPLACES param_special_s rather than adding one, so the count must stay 20
# -- 21 would mean an article param crept in, which is what Training Modpack's save state walks.
# And if the replace silently skipped (vlmerge without --replace just prints and returns), the
# object would still be Skull Bash's, every through_target lookup the victim's status makes would
# return 0, and the whipped opponent would go nowhere. Checking a key that only Incineroar's object
# has catches that.
# The param_special_s override and what is shipped must agree in BOTH directions. It is gated off
# in tools/gen-vlprc.sh because shipping it null-dereferenced smashline's load_fighter_effects on
# fighter load -- so a check that only fired when the file was PRESENT would have missed the case
# that actually happened.
want_vl=$(grep -cx 'SHIP_PARAM_SPECIAL_S=1' "$ROOT/tools/gen-vlprc.sh" || true)
if [ "$want_vl" != "0" ] && [ ! -f "$V" ]; then
  echo "  FAIL gen-vlprc.sh ships param_special_s but no vl.prc is present"; rc=1
elif [ "$want_vl" = "0" ] && [ -f "$V" ]; then
  echo "  FAIL a vl.prc is shipped but the override is disabled -- delete it or re-enable the gate"; rc=1
elif [ "$want_vl" = "0" ]; then
  echo "  ok  param_special_s override off, no vl.prc shipped (vanilla, no load-time risk)"
else
  n=$("$ROOT/tools/prc-tool/target/release/vlkeys.exe" "$V" 2>/dev/null | grep -c '^ *0x' || true)
  if [ "$n" != "20" ]; then
    echo "  FAIL shipped vl.prc has $n top-level entries, expected 20 -- something was ADDED, not replaced"; rc=1
  fi
  tts=$("$ROOT/tools/prc-tool/target/release/prcdump.exe" "$V" \
          "$ROOT/vendor/param-labels/ParamLabels.csv" 2>/dev/null \
        | grep -c 'special_s_through_target_speed = 11.2' || true)
  if [ "$tts" = "0" ]; then
    echo "  FAIL shipped vl.prc param_special_s is not Incineroar's -- the rope throw will not travel"; rc=1
  else
    echo "  ok  vl.prc carries Incineroar's param_special_s ($n entries, article count unchanged)"
  fi
fi


echo; echo "== 3f. a cloned article ships its own model and motion =="
# THIS IS THE CHECK THAT WOULD HAVE PREVENTED A CRASH. The clone was enabled with NONE of the
# spikeball's assets in the tree -- config.json never mentioned spikeball at all -- and loading into
# a match died in the game's own weapon-creation path. A cloned weapon is owned by pikachu, so the
# game looks for `fighter/pikachu/{model,motion}/spikeball/c00/...`, and there was nothing there.
#
# tools/gen-article-assets.sh copies or removes them to match CLONE_ENABLED, but nothing forced
# anyone to RUN it. This does: the tree and the constant must agree in both directions, because
# leftover assets with the clone off are dead declared paths in the search filesystem and missing
# assets with the clone on are a null dereference on load.
for kind in model motion; do
  d="$ROOT/pikachu-moveset/mod/fighter/pikachu/$kind/spikeball/c00"
  n=0; [ -d "$d" ] && n=$(find "$d" -type f | wc -l)
  if [ "$clone_on" != "0" ] && [ "$n" = "0" ]; then
    echo "  FAIL clone enabled but $kind/spikeball/c00 is empty -- run tools/gen-article-assets.sh"; rc=1
  elif [ "$clone_on" = "0" ] && [ "$n" != "0" ]; then
    echo "  FAIL clone disabled but $kind/spikeball/c00 still holds $n file(s) -- run tools/gen-article-assets.sh"; rc=1
  elif [ "$clone_on" = "0" ]; then
    echo "  ok  clone disabled, no $kind/spikeball assets shipped"
  else
    echo "  ok  clone enabled, $kind/spikeball/c00 ships $n file(s)"
  fi
done

echo; echo "== 3g. no shipped file is empty or implausibly small =="
# A ZERO-BYTE FILE ON THE CARD WAS BLAMED ON A CLONED ARTICLE FOR TWO CRASHES.
# `motion/body/c00/d03speciallwstart.nuanmb` uploaded truncated to 0 bytes and stayed that way
# across every deploy, because the verifier of the day checked presence with a directory LISTING
# and sys-ftpd truncates those under load. tools/deploy.sh now verifies by size and repairs, so the
# remote side is covered -- this covers the LOCAL side, where a generator that writes an empty file
# would otherwise sail through every other check here.
#
# The floor is per-extension and deliberately loose: it is looking for zero and near-zero, not for
# a file that is merely smaller than its neighbours.
bad_small=0
while IFS=$'\t' read -r sz path; do
  case "$path" in
    *.nuanmb|*.numdlb|*.numshb|*.nusktb|*.numatb|*.numshexb|*.bin|*.prc) floor=128 ;;
    *) floor=16 ;;
  esac
  if [ "$sz" -lt "$floor" ]; then
    echo "  FAIL ${path#"$ROOT/"} is $sz bytes (floor $floor)"; bad_small=$((bad_small+1)); rc=1
  fi
done < <(find "$ROOT/pikachu-moveset/mod" -type f -printf '%s\t%p\n')
[ "$bad_small" = "0" ] && echo "  ok  all $(find "$ROOT/pikachu-moveset/mod" -type f | wc -l) shipped files have plausible sizes"

echo; echo "== 3h. the in-game self-test walks the list this build ships =="
# The self-test's value is that it names motions the CONSOLE failed to resolve. A stale table would
# name motions the mod no longer ships -- reporting failures that are not real -- while saying
# nothing about ones it added, which is the worse half. tools/build.sh regenerates it before the
# compile; this catches a plugin built without running the build.
G="$ROOT/pikachu-moveset/plugin/src/telemetry/motions.rs"
want=$(grep -vE '^\s*#|^\s*$' "$ROOT/tools/ported-motions.txt" | cut -d: -f2 | sort -u)
have=$(grep -oE '^\s+"[a-z_0-9]+",' "$G" 2>/dev/null | tr -d ' ",' | sort -u)
if [ ! -f "$G" ]; then
  echo "  FAIL no motions.rs -- run tools/gen-selftest.sh"; rc=1
elif [ "$want" != "$have" ]; then
  echo "  FAIL motions.rs is stale -- run tools/gen-selftest.sh"
  diff <(echo "$want") <(echo "$have") | head -6 | sed 's/^/        /'
  rc=1
else
  # Every name it will ask the console about must be a name we actually put in motion_list.bin,
  # or an UNRESOLVED line on hardware would be the check's own bug rather than a real failure.
  # Step 3e already proves that: mlparity.exe reports "not comparable (we do not ship it)" per motion
  # and came back with none.
  echo "  ok  motions.rs matches ported-motions.txt ($(echo "$have" | wc -l | tr -d ' ') motions)"
fi
echo; echo "== 3d. no replaced move plays one of Pikachu's own animations =="
# The exhaustive version of the bug that made two of the three f-tilt angles keep his tail whip:
# a source fighter need not have every variant Pikachu has. Classifies each into REPOINTED (the
# shipped motion_list names a different file than vanilla), LOCKED OUT (only reachable through a
# status src/specials/lockout bounces out of), or GAP -- and only GAP fails.
VAR="$ROOT/tools/motion-patch/target/release/variants.exe"
"$VAR" "$ROOT/extracted" "$M" 2>/dev/null | tail -2
if "$VAR" "$ROOT/extracted" "$M" 2>/dev/null | grep -q 'GAP --'; then
  echo "  FAIL a replaced move still plays one of Pikachu's own animations"; rc=1
fi
# THE INVERSE, which is the check that had never existed: a motion the SOURCE has in a ported
# family and we have no entry for is a variant of the move that simply is not reproduced. That is
# how the airborne Alolan Whip and airborne Revenge went unnoticed -- six and three motions each,
# with the grounded animation and grounded hitboxes played in their place. Verified non-tautological
# by running it against vanilla Pikachu's list, where it reports all 22 synthesised entries.
if ! "$VAR" "$ROOT/extracted" "$M" 2>/dev/null | grep -q '^0 source motion'; then
  echo "  FAIL a ported move has a variant on the source fighter that this moveset does not have"; rc=1
fi

echo; echo "== 3e. each ported move carries its source's whole motion entry =="
# NOT JUST blend_frames, WHICH IS ALL THIS USED TO CHECK. The entry also carries `cancel_frame`
# (the FAF -- literally the frame data this project exists to reproduce), the intangibility window,
# `no_stop_intp` and the animation flag set. motion-patch patches all of them and nothing verified
# any of them; measured against vanilla the FAF patch alone moves 20 of the 46 comparable entries,
# by as much as 11 frames.
#
# blend_frames is the "seamless transition" requirement in numbers, and it was worth finding on its
# own: Palutena's catch_dash was snapping in over 0 frames instead of 3.
MOTIONS=$(grep -vE '^\s*#|^\s*$' "$ROOT/tools/ported-motions.txt" | tr '\n' ' ')
MLP="$ROOT/tools/motion-patch/target/release/mlparity.exe"
"$MLP" "$ROOT/extracted" "$M" $MOTIONS 2>/dev/null | tail -1
"$MLP" "$ROOT/extracted" "$M" $MOTIONS >/dev/null 2>&1 \
  || { echo "  FAIL a ported move's motion entry does not match its source's"; rc=1; }

echo; echo "== 3i. every ported move carries its source's damage and knockback =="
# check-hitboxes.sh measures WHERE a hitbox is and HOW BIG. This checks everything else the macro
# carries -- damage, angle, KBG, FKB, BKB, hitlag, SDI, collision attribute, sound level -- plus
# ATTACK_ABS, which holds the entire damage and knockback of the four DK cargo throws and has no
# geometry for the trace to see. "Exact frame data, damage, and range" is the project's own
# statement of intent, and only `range` had a gate.
bash "$ROOT/tools/check-acmd-values.sh" || rc=1

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
  air_speed_x_stable=1.332 air_speed_y_stable=1.63 dive_speed_y=3.097 air_accel_y=0.13 \
  landing_attack_air_frame_n=12 landing_attack_air_frame_f=10 landing_attack_air_frame_b=8 \
  landing_attack_air_frame_hi=9 landing_attack_air_frame_lw=14 || rc=1

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
