# ADR 0004 — Never hand a victim to engine code written for another fighter

Status: accepted, 2026-09-11.

## The decision

A port may not request a status on another battle object, or invoke engine code on one, unless
that code has been read and shown not to depend on something the source fighter has and Pikachu
does not. The refusal list and the validated handle live in
`pikachu-moveset/plugin/src/victim.rs`; `tools/check-victim.sh` enforces both.

## Why

Every crash this moveset has produced is one mistake in different clothes. The clearest is
proven twice over — by an Atmosphere report and by the game's own binary:

```
Data Abort, fault address 0
  L2CFighterCommon::status_SwingGaogaenThrown + 0xf8
  -> app::lua_bind::LinkModule__get_parent_pos_impl
```

`status_SwingGaogaenThrown` fetches the position of its **linked parent** and reads x, y and z
off the returned pointer with no null check, because for Incineroar the parent is always his
ROPE article. Pikachu has no rope, so there is no link, so the pointer is null and the third
load is the fault address. The whip was requesting that status on its victim as the "faithful"
thing to do — the outbound leg, the bounce and the return are all engine-common statuses, so
handing them over looked like using the game's own code rather than reimplementing it.

It was the opposite. A common status is not fighter-agnostic just because every fighter can
enter it.

## What makes this checkable rather than a judgement call

The same binary contains the contrast. `status_SwingGaogaenCatched` calls
`LinkModule::is_link` **first** and only then touches the parent, so with no rope it simply does
not constrain the victim's model — which is exactly what a port that drives the victim itself
wants. Two statuses, one file, opposite answers.

So safety is a property of the individual status, discovered by reading it, and the outcome of
that reading belongs in a list rather than in someone's memory.

## How to read one

`prebuilt:/nro/release/lua2cpp_common.nro`, pulled from the running game with
`tools/arc-pull` (8.4 MB, 8792 defined symbols — every status the roster shares), then:

```bash
tools/disasm/target/release/disasm.exe <nro> --sym <StatusName>
tools/disasm/target/release/disasm.exe <nro> --calls <addr> 500
```

What disqualifies a status:

* any `LinkModule::get_parent_*` that returns a **pointer** (`get_parent_pos`) without an
  `is_link` guard ahead of it;
* any `WorkModule::get_param_*` against a param object the host fighter's `vl.prc` does not
  have — the lookup returns 0 rather than crashing, so the move silently does nothing;
* any article access at all.

What is known safe, and used: `SWING_GAOGAEN_CATCHED` (guarded), `CAPTURE_PULLED`
(`CaptureModule::update_node_pos`, `motion`, `motion_lw` and nothing else).

## Consequences

The Alolan Whip drives the victim's trajectory itself with Incineroar's own
`special_s_through_target_*` numbers. That was already necessary for a second reason — Pikachu's
`vl.prc` has no `param_special_s` for those statuses to read — so nothing is lost but the visible
ropes.

The cargo carry now leaves the victim's position to `CaptureModule` for as long as the engine's
capture still names Pikachu as master, and only writes their posture once it has let go. That
removes the last cross-object write from the carry.

And the second half of the rule: a handle to another battle object is validated, not assumed.
`crate::victim::boma` requires the id to be shaped like `entry << 16`, to be live, and for the
accessor to **agree about its own `battle_object_id`**. A stale id — `capture_object_id` read one
frame too late, which is what the cargo path was doing — resolves to a pointer the engine will
happily make a virtual call through, and the resulting Instruction Abort names no function at
all. That is the least diagnosable failure this project has had, and it is now unreachable.

## Amendment, 2026-09-11: the parent is the CAPTOR, not the rope

The decision stands and the refusal list stays. One premise in the reasoning above was wrong, and
correcting it changes what is possible rather than what is allowed.

`status_SwingGaogaenThrown`'s unguarded `get_parent_pos` asks for `LINK_NO_CAPTURE`:

```
0x710028c504  ldr w1, [x26, #0x4e4]   ; ~lua_const LINK_NO_CAPTURE
0x710028c520  ldr x0, [x19, #0x40]
0x710028c524  bl  lua_bind::LinkModule__get_parent_pos_impl
0x710028c528  ldr s0, [x0]            ; <- the fault
```

`LINK_NO_CAPTURE` is the CAPTURING FIGHTER, not the rope article. Two independent uses settle it:
`status_SwingGaogaenCatched` resolves the same link with `get_parent_id` and then reads
`param_special_s` off the result, and those are Incineroar's params, not a rope's.

So the pointer was null for a simpler reason than "Pikachu has no rope". There was no capture at
all. The port put the victim into SWING_GAOGAEN_CATCHED with `change_status_request_from_script`
and never called `CaptureModule::capture`, because the ACMD's CATCH collision connects but the
capture bookkeeping is fighter-kind-specific native code that does not run for a Pikachu. The
trace said so on every frame and it was read as noise: `cap=0x50000000` is
BATTLE_OBJECT_ID_INVALID, and the victim's side reported `cap=false`.

`specials::take_captive` now makes that call, so the link exists and those three statuses are no
longer null-dereferences waiting to happen.

**They are still not used, and the reason is now a different one.** The victim's own catch status
reads `FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLAG_SPECIAL_S_GENERATED_ROPE` off the captor to decide
between the rope path and the no-rope path. That constant is `0x200000EA` — instance work flag
index 234 — and `FIGHTER_PIKACHU_INSTANCE_WORK_ID_FLAG_TERM` is `0x200000E8`, so Pikachu's flag
array ends at 232. The flag reads 0 for a Pikachu captor, the engine takes the no-rope branch, and
the no-rope branch is `special_s_through_target_speed_no_rope` 1.6 with no bounce at all — against
11.2 out and 2.8 back with one. Playtest has the hand-driven version bouncing correctly ("I can
get the hit to land after the bounce on the ropes"), so handing the flight to the engine would
trade a working move for a slower one with the signature hit missing.

The event that would start it is identified and recorded rather than sent:
`hash40("gaogaen_swing_throw")` = `0x13b7318a5a`, the whip's counterpart of Donkey Kong's
`shoulder_throw`. See `vendor/main/hash40-confirmed.txt`.

So the rule for these three becomes: **not without a capture link, and not until Pikachu can carry
the rope flag.** The second half is an article problem, which is where `tools/gen-vlprc.sh` and
`src/articles/mod.rs` already point.
