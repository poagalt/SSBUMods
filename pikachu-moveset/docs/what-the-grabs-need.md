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
| 3 | The catch connects | **native** — `macros::CATCH(.., FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED, ..)`. The handler is generic; the target status is an argument |
| 4 | The captor enters `SPECIAL_S_THROW` | *substituted* — `whip/catch.rs`. This transition IS kind-keyed and does not run for a Pikachu. The one irreducible substitution |
| 5 | The capture is recorded | **native** — `CaptureModule::capture`, adopted from the engine when it did it first |
| 6 | The link exists | **native call, port-issued** — `LinkModule::link` on BOTH objects; `Link(this, no, id)` writes only the caller's slot |
| 7 | The victim enters `SWING_GAOGAEN_CATCHED` | **native status, forced entry** — `change_status_force`; an unforced `_from_script` request is declined |
| 8 | The victim is positioned on the captor | **native** — their own status does `set_model_constraint_pos_ort(LINK_NO_CAPTURE, "rot", "throw", POSITION, ORIENTATION)` |
| 9 | The victim faces the right way | **native** — their status: `get_parent_lr` → `set_lr` → `update_rot_y_lr` |
| 10 | The victim's clatter uses OUR numbers | **native** — their status reads the captor's `param_special_s`; `src/params` answers it |
| 11 | No rope exists, and the victim knows | *intercepted* — `is_flag(GENERATED_ROPE)` answered `false`. The id is 0x200000EA, two past Pikachu's array, so it cannot be written |
| 12 | The victim flies out and returns | **native** — their `THROWN` status, driven by `special_s_through_target_*_no_rope` |
| 13 | The throw event reaches them | **native** — `send_event_nodes(LINK_NO_CAPTURE, hash40("gaogaen_swing_throw"))`, confirmed in the recording |
| 14 | The follow-up areas | *substituted* — ADR 0004: the rope raises these flags and there is no rope. Geometry from the params (`-24`, `-10`, `-6`, `node=top`), gated on a `LARIAT_BEEN_OUT` latch |
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
| 8 | Walking while carrying | **native** — `sub_walk_uniq_process_init_common(fast, middle, slow, 0)`, using Pikachu's own walk params |
| 9 | The throw direction | **native** — the CAT2 pad command flags, with a raw-stick fallback for a press with the stick already held |
| 10 | The throw is refused if they mashed out | **native** — `get_clatter_time` on the victim |
| 11 | The throw event | **transcribed** — `send_event_nodes_throw`, four-way motion map confirmed by the recording |
| 12 | The release | **transcribed** — `CatchModule::catch_cut` on ourselves. **Not** the whip's `whip_release` |
| 13 | The victim's position each frame | **native, with a self-disabling fallback** — see below |

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
