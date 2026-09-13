# What a grab needs, and where each piece comes from

Written after the whip's third failed playtest, because "port the native implementation" had been
applied to the captor's status tree and to nothing else — and every remaining bug lived in the
parts nobody had enumerated.

**A grab is two fighters.** Everything below is required for one to work. The column that matters
is the third: anything not `native` or `transcribed` is a place this port can be wrong, and the
list of those is short enough to read.

## Alolan Whip

| # | What | Where it comes from |
| --- | --- | --- |
| 1 | The lunge slides forward | **native** — ACMD raises `FLAG_MOVE_START`/`_MOVE_END`, `whip_kinetics` reads `FLAG_MOVE` |
| 2 | The MOVE edges become the steady flag | *substituted* — source installs the toggler as a callback in `table[0x15]`; the port calls it per frame from the loop. Equivalent because the toggler is edge-triggered and clears what it acts on |
| 2b | The lunge NUDGES them before it grabs | **native, and reported as a bug** — "point blank alolan whip doesn't immediately grab the opponent, it pushes them back a bit then grabs them". `game_specialsstart` fires two 0-damage hitboxes on frames 15 and 16 — `ATTACK(.., 0.0, 350, 100, 30, 0, 5.0, ..)`, fixed knockback 30, one masked to `FIGHTER` and one to `FEB` — and the `CATCH` capsules only come out on frame 18. The push is the move |
| 3 | The catch connects | **native** — `macros::CATCH(.., FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED, ..)` on frames 18 to 35, which is Incineroar's own window. The handler is generic; the target status is an argument |
| 4 | The captor enters `SPECIAL_S_THROW` | *substituted* — `whip/catch.rs`. This transition IS kind-keyed and does not run for a Pikachu. The one irreducible substitution |
| 5 | The capture is recorded | **native** — `CaptureModule::capture`, adopted from the engine when it did it first |
| 6 | The link exists | **native call, port-issued** — `LinkModule::link` on BOTH objects; `Link(this, no, id)` writes only the caller's slot |
| 7 | The victim enters `SWING_GAOGAEN_CATCHED` | **native status, forced entry** — `change_status_force`; an unforced `_from_script` request is declined |
| 8 | The victim is positioned on the captor | **native** — their own status does `set_model_constraint_pos_ort(LINK_NO_CAPTURE, "rot", "throw", POSITION, ORIENTATION)` |
| 9 | The victim faces the right way | **native** — their status: `get_parent_lr` → `set_lr` → `update_rot_y_lr` |
| 10 | The victim's clatter uses OUR numbers | **native** — their status reads the captor's `param_special_s`; `src/params` answers it |
| 11 | A rope exists, and the victim is told where | *intercepted* — `is_flag(GENERATED_ROPE)` answered **`true`**, and `get_float(SPECIAL_S_FLOAT_ROPE_X)` answered with `pos_x + lariat_offset_x1 * lr`. Both ids are unwritable on a Pikachu (0x200000EA is two past his instance array; 0x01000007 is a status-scoped INDEX, so it means something else in every other status -- see `src/params::in_whip_status`). Answering `false` was honest about the article and wrong about the move: the no-rope branch is a SEVEN TIMES SLOWER throw that stops on a frame count instead of at a place |
| 12 | The victim flies out and returns | **native** — their `THROWN`, `ATTACH_ROPE` and `RETURN` statuses, driven by `special_s_through_target_speed` (11.2), `_speed_over_rope` (1.6) and `_return_speed` (2.8) now that row 11 answers `true` |
| 12b | Each leg of the flight ends | **native counts, port-issued** — `_motion_frame_fix` = 8 for the outbound leg and `_return_motion_frame_fix` = 8 for the return, both matching three native whips to the frame. The rope's 20-frame HOLD is the one measured constant in the whip: it is in no param because it belongs to the article |
| 12c | The victim is snapped onto the ropes | **native status, forced entry** — `SWING_GAOGAEN_ATTACH_ROPE`. It was on `FORBIDDEN_VICTIM_STATUSES` for six rounds on the assumption that "the rope IS that status"; its three functions are `init_settings`, `set_swing_special_state`, `change_motion`, `set_rate`, `GroundModule::correct` and one `LinkModule::is_link`, and nothing in them touches a rope. Natively the rope's own event moves them in and out; the port supplies both ends |
| 13 | The throw event reaches them | **native** — `send_event_nodes(LINK_NO_CAPTURE, hash40("gaogaen_swing_throw"))`, confirmed in the recording |
| 14 | The follow-up areas | *substituted* — ADR 0004: the rope raises these flags and there is no rope. Geometry from the params, now read in full: three VERTICAL TRIPWIRES on `top` at x = -24, -10 and -6, each spanning y = 2..12 with radius 2, crossed on the way out and again on the way back. Gated on a `LARIAT_BEEN_OUT` latch. Three native whips recorded `area L=0 Lend=0 F=0` on every frame while Incineroar still reached LARIAT and SHOULDER, so whatever raises them for him is the article's and this geometry is the stand-in |
| 15 | Early press → shoulder, late press → lariat | **transcribed** — the THROW loop's own decision tree |
| 16 | The lariat's animation speed | **transcribed** — `lo + (hi - lo) * |push_x - hit_x| / area_length` |
| 17 | Kinetics for all six statuses | **transcribed** — one function, gaogaen `0x710001d4b0` |
| 18 | Air drift after the move | **transcribed** — `0x710001ff80`, the fighter's own air stats times two whip params |
| 19 | The release | **transcribed** — `0x71000206c0`. Incineroar tears down the LINK; he does not cut the catch |
| 20 | The victim escapes on a mash | **native** — their loop: clatter runs out → `CAPTURE_CUT` |

## Cargo carry

| # | What | Where it comes from |
| --- | --- | --- |
| 1 | Setup on entering the carry | **transcribed, now on the right line** — DK's `SHOULDER_START` `INIT_STATUS`, registered as `Init` not called from `Main` |
| 2 | Teardown on leaving it | **transcribed, now on the right line** — the shared `EXIT_STATUS` `0x7100004df0`, registered as `Exit` so it runs on EVERY exit including a hit |
| 3 | Which throw motion the victim plays | **transcribed** — that same helper, keyed on `global_table[0xb]`, the status being entered |
| 4 | The capture and link | **native calls, port-issued** — as whip 5/6 |
| 5 | The victim's lift animation | **native status** — `ShoulderedDonkeyStart` |
| 6 | The victim's carried pose | **native status** — `ShoulderedDonkey`: `shouldered_donkey` motion, constraint offset, `CameraModule::set_being_carried` |
| 7 | The victim's mash decay | **native** — `exec_fix_pos_counter`, which reads the CARRIER's situation kind |
| 8 | Walking while carrying | **native** — `sub_walk_uniq_process_init_common(fast, middle, slow, false)` on `Init` AND `sub_walk_uniq_process_main_common(.., NIL)` on `Exec`, using Pikachu's own walk params. The `Exec` line was missing for six rounds, so the three-speed walk chose its speed once and never again |
| 9 | The throw direction | **native** — the CAT2 pad command flags only. The raw-stick fallback that used to sit behind them compared against +/-0.5, three numbers from nowhere; DK reads `IsThrowStick` to know whether a direction was asked for and treats no-flag-no-deflection as forward |
| 10 | The throw is refused if they mashed out | **native** — `get_clatter_time` on the victim |
| 11 | The throw event | **transcribed** — `send_event_nodes_throw`, four-way motion map confirmed by the recording |
| 12 | The release | **transcribed, plus port-side symmetry** — `CatchModule::catch_cut` on ourselves, and only when `CatchModule::is_catch` agrees there is one to cut, because the call is a REQUEST that survives to the next catch if nothing consumes it. Plus the two things this port has to undo because it made them by hand: `LinkModule::unlink` (guarded on `is_linked` -- unlinking an empty link is a crash) and CAPTURE_CUT on a victim the engine still records as ours. All of it in `victim::drop_capture`, reached from `leave_carry`, `throw_exit` and `whip_release` |
| 13 | The victim's position each frame | **native, with a self-disabling fallback** — see below |
| 14 | The throw's hitbox, damage and camera | **native** — `game_throwff` / `_fb` / `_fhi` / `_flw`: `ATTACK_ABS(THROW, 12.0, angle 60, kbg 46, bkb 90)`, `ATTACK_ABS(CATCH)`, `CHECK_FINISH_CAMERA(38, 19)` and `ATK_HIT_ABS` on frame 15 against the three `THROW_WORK_INT_TARGET_*` ints |
| 15 | Who the throw is aimed at | **transcribed, on `Init`** — DK's `ThrowF*_init` reads the throw's motion out of `FIGHTER_STATUS_CATCH_WAIT_WORK_INT_MOTION_KIND` (which row 3 wrote) and takes the target from `LinkModule::get_node_object_id`. Our own `VICTIM[entry]` is the fallback for the no-link case |
| 16 | The throw ends when the player can act | **native** — `CancelModule::is_enable_cancel` then `sub_wait_ground_check_common` / `sub_air_check_fall_common`, which is DK's whole `ThrowF*_main_loop`. The FAFs are 46/43/30/45 against animations of 49–59, and a brand-new status id is not in the engine's throw family, so nothing was reading them |
| 17 | Jumping out of the carry | **transcribed** — `FUN_7100023820` on `Wait`/`Walk`/`Turn`, plus `SHOULDER_JUMP_SQUAT`, `_JUMP_SQUAT_B`, `_JUMP` and `_PASS` and the short-hop helper `FUN_7100022f30`. Reads `common/jump_stick_y`, `jump_flick_y`, `jump_neutral_y` off whoever runs it |
| 18 | Turning while carrying | **transcribed** — `common/turn_frame` into `TURN_WORK_FLOAT_TURN_FRAME`, `off_flag(TURN_FLAG_TURN)`, and `FUN_7100023390` counting it down each frame until it flips `lr` once. Without the countdown the turn animation played and `lr` never changed, so WAIT turned him again forever |
| 19 | Which stick deflection means what | **native** — `global_table[0x1a] * lr` against `common/turn_stick_x` and `common/walk_stick_x` with the operands swapped, which is how one slot and two params give a signed window. All three params live in `fighter/common/param/common.prc`, game-wide, so Pikachu gets DK's numbers by construction |
| 20 | How long the hold may last | *substituted* — twice the victim's own `get_clatter_time`, re-derived every frame and only ever moving forward, floored at 170. A flat ceiling cut every carry at 171 frames on a victim with 425 of 426 left |

## Still substituted, and what it would take to retire each

- **Whip 4 (the captor's THROW transition).** Irreducible: the native handler is keyed on fighter
  kind. Nothing to do.
- **Whip 14 (the areas).** Needs the rope article, which ADR 0004 excludes. Retiring it means
  reopening that decision, not writing better code.
- **Whip 2 (the MOVE callback).** Needs `table[0x15]`'s semantics — what slot it is and when L2C
  fires it. A deeper reverse-engineering job than the behaviour justifies, and the equivalence is
  argued in the code.
- **Whip 11 (`GENERATED_ROPE`).** Permanent while ADR 0004 holds. The interception is the honest
  answer, not a workaround.
- **Cargo 13 (victim position) — RETIRED, and it was never really substituted.** I first wrote this
  row as a substitution to be fixed by finding the bone DK's carry constrains to. There is no such
  bone. `set_model_constraint_pos_ort` has 22 call sites in `lua2cpp_common` and not one of them is
  in the `ShoulderedDonkey` family; `lua2cpp_donkey` does not import the function at all. The
  constraint is created by native `main` inside the capture itself, and `status_ShoulderedDonkey`
  only adjusts its flags (`CONSTRAINT_FLAG_OFFSET_ROT`, `set_constraint_rot_offset_y`). That is
  also why `CaptureModule::set_nodes` never appears in `docs/the-recording.md`.

  So the engine positions the carried fighter, and `shoulder::hold_victim` already knew: it returns
  early when `is_capture` is true and we are the master, and only writes their posture as a
  fallback once the engine has let go. With the capture now properly established that fallback
  should stop firing — which is checkable, because the function logs which of the two is driving.

## Two things that are wrong and known

- `throw_invincible_frame` reads 0. The parameter is in none of the extracted files. See
  `docs/the-carry.md`.
- The whip does not get its own ledge-grab box while it runs: `select_cliff_hangdata`'s argument is
  `FIGHTER_GAOGAEN_CLIFF_HANG_DATA_SPECIAL_S`, a fighter-scoped index ADR 0006 forbids reusing.

## The rule this document exists to enforce

Every row above was found by reading the game, and **every bug in the last four playtests was a row
nobody had written down**: the ACMD flag that `deforeign` stripped, the victim's status that was
never entered, the link that was only half made, the article generator that spawned a Thunder Jolt.
None of them were in the captor's status tree, which is the only part ADR 0006 told anyone to
transcribe.

So the rule is: **before claiming a move is ported, enumerate what it needs and say where each
piece comes from.** A transcription of one fighter's status functions is not a port of a move.
