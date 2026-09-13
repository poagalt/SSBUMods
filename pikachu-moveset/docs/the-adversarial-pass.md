# The adversarial pass

A deliberate attempt to break this port against its own premise: that every move is the source
fighter's move, not an imitation of it. Written after the session that brought both grabs into a
working state, because "it works now" is the moment a port stops being examined.

Where this page and a comment in the code disagree, the code's comment is probably newer -- but
where this page records a MEASUREMENT, that measurement stands until a new one replaces it.

## What the pass added, and why a check rather than a paragraph

Three gaps were found by playtest during this project, one at a time, over four sessions. All three
were the same shape: a thing the source has that the port does not, invisible to every existing
check because the existing checks compare what EXISTS against the source and say nothing about what
is absent.

| | what was missing | how it looked in game |
|---|---|---|
| `ShoulderWalk`'s `Exec` | `sub_walk_uniq_process_main_common` | the carry walked one direction only |
| the carry's shared `Exit` | `catch_cut` + `ignore_catching` teardown | victim stuck when the engine ended the status |
| the four throws' `Exit` | `CatchModule::catch_cut` | Pikachu could not grab again after a cargo throw |

So the pass is three checks, wired into `tools/check.sh` as steps 4g, 4h and 4i:

* **`check-scripts.sh`** -- every `game_` script a shipped motion NAMES must be installed. Reads
  the shipped `motion_list.bin`, which is what the ENGINE reads, rather than either of the port's
  own lists.
* **`check-lists.sh`** -- `gen-moves.sh`'s inline tables, `tools/moves.tsv` and `topscale`'s `MAP`
  must agree. Three hand-maintained lists that had drifted in all three directions.
* **`check-statuslines.sh`** -- every `Init`, `Exec` and `Exit` line a ported status has in the
  source DECOMP must be registered. The decomp tree contains one `.c` per line the status actually
  has, so the presence of `exec.c` IS the fact that the status has an `Exec`.

Each was self-tested by breaking the thing it watches, and two of them failed that self-test on the
first attempt -- `check-scripts` walked `moves.tsv`, so deleting a row made it pass, and
`check-victim`'s rewritten step 1 reported ok in both directions because `grep -c || echo 0` prints
`0\n0`. A check that cannot fail is worse than no check.

## What they found

* `game_speciallwturn` and `game_specialairlwturn` -- Revenge's turn-around retaliation, two
  `ATTACK`s and a `REVERSE_LR`, named by two motions whose animations had been shipping since the
  counter was ported and installed by nothing. Adding them then walked into `topscale`'s `MAP` and
  produced `0.44 world units off`, the same signature that list produces every time it is out of
  step.
* Fox's three dodge scripts, generated and absent from the manifest, so never measured.
* **All four cargo throws have an `init.c` and the port registered no `Init`.** The body was in
  `main`, one status line too late -- and porting it properly closed a loop the port had left open:
  DK's init reads the throw's motion out of `FIGHTER_STATUS_CATCH_WAIT_WORK_INT_MOTION_KIND`, which
  `leave_carry` had been writing all along with nothing reading it.

## Frame data and timing, measured against native play

From `work/pt18.log`, three native Incineroar whips that agree to the frame:

```
f1   victim 0x176 CATCHED      off=+14.41   rope flags 00
f36  victim 0x177 THROWN       off= -7.09   rope flags 01
f44  victim 0x178 ATTACH_ROPE  off=-41.55   rope flags 01
f64  victim 0x179 RETURN       off=-46.59   rope flags 01
f73  follow-up                 off= +3.09
```

Which settled four things that had been guesses:

* the throw fires on captor animation frame 36, matching `game_specialsthrow`'s frame-37
  `on_flag(FLAG_THROW_ROPE)`;
* the outbound leg is 8 frames, which is `special_s_through_target_motion_frame_fix` = 8;
* the return leg is 8 frames, which is `_return_motion_frame_fix` = 8;
* the rope holds them for **20 frames**, which is in no param -- it belongs to the rope article --
  and is the one measured constant in the whip.

And two things that were wrong in the port: `special_s_lariat_offset_x1` = -24 is not the rope (the
rope catches them 17 units further out), and the three area params are vertical tripwires on `top`
at x = -24, -10 and -6 spanning y = 2..12 with radius 2, crossed on the way out and again on the
way back.

The cargo throws' first-actionable frames come from the shipped motion list -- `throw_f_f` 46,
`throw_f_b` 43, `throw_f_hi` 30, `throw_f_lw` 45, against animations of 49 to 59 -- and are now
honoured through `CancelModule::is_enable_cancel` plus `sub_wait_ground_check_common` /
`sub_air_check_fall_common`, which is DK's own loop. A brand-new status id is not in the engine's
throw family, so nothing was reading the FAF at all.

## The mirroring bug: four causes eliminated, one instrument shipped

Chrom's jab and Palutena's n-air do not mirror when Pikachu faces left. They are the only two
ported moves whose hitboxes ride a GRAFTED bone -- `Sword1` and `Stick` -- which is the entire case
against the graft, and every static fact about it has now been checked and clears it:

* the shipped `model.nusktb` has `Sword1` at index 78 and `Stick` at 79, both `parent_index = 22`,
  and index 22 is `HaveR`, whose chain runs up through `ArmR`;
* both motion entries carry exactly the source's flags (`attack_11` `move` set, `attack_air_n`
  clear);
* `tools/reach reanchor`, which would bake a bone offset into `top`, is disabled and not in the
  build;
* **the FK parity harness measures `nair_palutena` at 0.00 deviation over 96 samples with `stick`
  as the anchor, and `jab_chrom` at 0.00 over 9.** The grafted bones are driven exactly where the
  source's are.

So the graft is innocent and the cause is at hitbox creation or in the visual, not in the skeleton.
`telemetry::mirror_probe` logs the anchor bones' offset from `top` in the fighter's own frame, once
per facing, with `haver` beside them as the control. If the grafted bones flip sign between facings
and `haver` does not, it is still the graft by some mechanism not yet found; if `haver` flips too,
the graft was never involved.

## What cannot be delivered under the standing decisions

**The Alolan Whip's ropes have no sprite, and they cannot have one without reversing an ADR.**

The ropes are `FIGHTER_GAOGAEN_GENERATE_ARTICLE_ROPE`, an article. ADR 0004 excludes articles
because article ids are per-fighter enumerations -- generating Incineroar's rope id on Pikachu
aliases onto whatever sits at that index, which is how an earlier attempt turned the rope into a
Thunder Jolt. ADR 0002 restricts effects to `sys_*`, and there is no `sys_` effect that reads as a
rope.

What the port supplies instead is everything the rope DOES: the coordinate the victim flies to
(answered through `src/params` as `SPECIAL_S_FLOAT_ROPE_X`), the flag that selects the rope branch
of the victim's own status, the entry into `SWING_GAOGAEN_ATTACH_ROPE`, and the 20-frame hold. The
physics are the game's; the visual is absent.

The same class applies to Game & Watch's up-B: `game_specialhiopen` is defined under
`lua2cpp_gamewatch/gamewatch_parachute/`, and his status set has no `SpecialHiOpen` at all -- the
open parachute is a state of the ARTICLE. `RAT_STATUS_KIND_SPECIAL_HI_OPEN` is this port's own
construction standing in for it, and `check-statuslines.sh` records that rather than pretending
there is a decomp to compare against.

Shipping either article means the `CLONE_ENABLED` path in `tools/build.sh`, which exists and is
off. That is a decision, not an oversight.
