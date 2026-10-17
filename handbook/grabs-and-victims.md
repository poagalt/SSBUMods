# Command grabs — the hard class

A grab is the only kind of move where **another fighter's code runs on your state**. Nine of the
ten longest investigations in this project's history were grabs, and every one of them reduces to
the same sentence, which took months to be able to write:

> I ported the attacker and forgot the victim.

A command grab is two fighters running two different pieces of the game's code. The captor's half
is in `lua2cpp_<source>.nro`. The victim's half is in `lua2cpp_common.nro`, and it is the half
that positions them, animates them, starts their mash timer, and releases them.

## The victim's half

The victim-side statuses are **engine-common**: `SWING_GAOGAEN_*` (`0x176`–`0x17C`),
`SHOULDERED_DONKEY_*` (`0xC5`/`0xC6`/`0xC7`), `CAPTURE_PULLED` (`0x41`), `CAPTURE_WAIT` (`0x42`),
`CAPTURE_CUT` (`0x44`), `CAPTURE_JUMP` (`0x45`). Common means every fighter *can* enter them. It
does not mean they are fighter-agnostic, and the difference has cost this project three crashes.

**They read their numbers off the captor.** `status_SwingGaogaenCatched` does not carry
Incineroar's values; it reads `special_s_through_target_speed` (11.2), `_return_speed` (2.8), and
`_cancel_frame` (30) out of the **grabber's** `param_special_s`, on every catch. Pikachu already
has a `param_special_s` — Skull Bash's — which contains none of those keys, so every lookup
returned 0 and the whipped opponent went nowhere no matter how correct the captor's status machine
was. That is what the param provider in `src/params/` exists for.

**They read flags off the captor by instance-work index.** The victim checks a rope flag at
instance-work index 234 on the captor; Pikachu's array ends at 232. So the engine takes the
no-rope path — 1.6 units per frame with no bounce, against 11.2 out and 2.8 back. Handing the
flight to the engine would trade a working bounce for a slower move with the signature hit
missing. That is the current, documented reason ADR 0004 stands, and it replaced the ADR's
original reason, which turned out to be wrong.

**They have to be told which pose to take.**
`LinkModule::send_event_nodes(boma, LINK_NO_CAPTURE, hash40("shoulder"), 0)` is how DK tells the
fighter on his shoulder to be carried. Without that event the victim keeps whatever pose the grab
gave them — which is the flailing fall animation that read in playtests as "Mario looks like he's
in free fall while being carried".

**A T-pose is a fighter with no valid motion.** A victim still constrained to the captor's bone
with no status to play is exactly that. Release has to leave them in a status that has a motion —
requesting `CAPTURE_CUT` is what the engine's own escape does, and it produces the "popped out
like from a normal grab" behaviour.

## Making and unmaking a capture

This is the part where every intuition was wrong, in order.

**The grip is free.** `LinkModule::set_model_constraint_pos_ort` binds the victim's `"rot"` to the
captor's **`"throw"` bone** the moment a capture exists. Hand-positioning a held victim by writing
its position each frame is not necessary, and doing it is what glued opponents to Pikachu's feet.

**Neither source fighter ever calls `CaptureModule::capture`.** For DK and for Incineroar the
engine establishes the capture from the collision, in kind-keyed native code that passes through
no hookable module function — so there is nothing to transcribe there. DK's link event alone moves
the victim into `0xC5`.

**And a capture a plugin makes by hand cannot be undone by anything the plugin can call.** A
release ladder was tried, step by step, and the definitive result was that there is no release
call: native DK's clean state reads `captured_by = 0x50000000` while a by-hand capture leaves
`0x0` standing forever. **The fix is not a missing function — it is to not make the capture at
all**, and to enter the carry the way DK does.

**Which is a one-line retarget.** `L2CFighterCommon::CatchCont` tests `THROW_B`, `THROW_HI`,
`THROW_LW`, then `THROW_F`, and the forward throw does not go to a hardcoded status — it goes to
**`globalTable[0x45]`**. DK puts `SHOULDER_START` there. So the carry is entered by writing your
own status id into that slot and letting the engine make the transition, and nothing ever tears
the grab down because nothing ever built one by hand.

**`LinkModule::link` writes only the caller's slot** (`+0x30` parent, `+0x48` id) and splices the
caller's two lists. It does not touch the named object, which is why a by-hand capture needs two
calls — and why leaving a node on `LINK_NO_CAPTURE` makes the engine believe you are still holding
someone, which blocks the next grab entirely.

**What `capture` actually does**, read out of the 13.0.5 decompile (`main` `0x3ede00`, the target
of `vtable[0x80]` — a function nothing `bl`s to, so it was unreachable until the boundary list
existed). Two gates, in order:

1. `uVar5 = vtable[0x48](accessor, 0, captor_id)` — **the entire body is gated on this returning
   true.** Reads like a "can this object be captured by that one" test *(inference — the call is
   virtual and unnamed; what is certain is that nothing below runs without it)*.
2. It then builds a capture record on the stack and passes it to `vtable[0xa8]`. If a byte in that
   record is still zero afterwards, it calls `vtable[0x68]` and **returns 0**.

The record's contents are legible, and two fields are hash40s:

| Field | Value |
|---|---|
| + | `hash40("capture")` = `0x78bfea6e5` |
| + | `hash40("invalid")` = `0x7fb997a80` |
| + | `CONCAT44(0xffffffff, victim_own_id)` — `0xffffffff` is **`STATUS_KIND_NONE`** |
| + | the `status_kind` argument the caller passed |
| + | `0x1000001`, `10`, and two pointers |

`hash40("invalid")` is the constant `CaptureCut` compares `param_1[0x70] & 0xffffffffff` against,
so **field `+0x70` is a hash40 slot whose unset value is `"invalid"`** — which is the missing half
of why `capture_cut(true, true, false)` is the working combination: `p4 == false` short-circuits
that hash test before it is reached. And the `0xffffffff` in the record is the same
`STATUS_KIND_NONE` the constant table gives for `lua_const[0x250]`, arrived at from the other
direction.

This also gives **open question 3** a shape: "capture returned without capturing" means one of
those two gates failed, and the function's own return value distinguishes them.

**Release, precisely.** `CatchModule::catch_cut` is a request (`this->[0x93] = 1`) the engine
consumes only while a catch is active; raised with nothing to cut, it survives to the next catch.
`CaptureModule::capture_cut(v, p2, p3, p4)` gates the vtable call that actually undoes the capture
on **`p2 && p3 && !p4`**. For five rounds a comment in this repo said the gate was `p2` alone, so
the ladder tried `(true,false,false)` and `(true,true,true)` and **never tried `(true,true,false)`**,
which is the one that works.

**Timing beats state.** A cut issued while the victim is mid-throw (status `0x49`) interrupts the
throw. What separates the two cases is *time*, not state, so the release records the victim's
status and arms; `tick_deferred_cut` fires on the first frame that status is something else, with
a `2 × MAX_HOLD_FRAMES` deadline as a backstop. No status id has to be identified — only that it
changed.

**And DK's own throw exit is one line:** `CatchModule::catch_cut(false, false)`. No forced
`CAPTURE_CUT`, no `capture_cut`. Capture surgery is correct for a hand-made capture and wrong for
an engine-made one, so the two paths must be split.

## Diagnosing a grab

**A blocked grab is not a whiff, and reading it as one cost several rounds.** Pikachu's catch
collision is on frames 6–7 of a 50-frame animation, so the CATCH status's *duration* tells you
which happened: a whiff runs the full 40–51 frames, while a catch that is cut on contact ends in
8–13. Same symptom on screen, opposite causes.

**`CaptureModule::is_capture` is true on the captor too** — measured in 10 of 12 probes. A guard
written as "if I am not capturing, bail" fires exactly when the release is needed.

**A "nobody" sentinel of `0` is player one.** Battle object id `0x0` is a valid id (entry 0), so a
`HELD_ID` initialised to zero made every `their_*` diagnostic read off Pikachu himself for three
rounds. The sentinel is `u32::MAX`.

**`check_button_on` is held, not pressed.** The grab button still being down committed a throw on
the first carry frame. `check_button_trigger` is the edge.

**`IsThrowStick()` returns an L2C table** indexed by `hash40("f"/"b"/"hi"/"lw")`, not a bool.

**The throw input is a mandatory AND.** DK's input helper computes four cat2 masks and **one** cat1
mask and combines them with a single `operator&` against `globalTable[0x20]` — so the *button* is
mandatory and the stick only picks the direction. A cat2 flag alone committing a throw means
walking throws the victim, because those flags fire on a stick flick.

## The rope, and why the whip's flight is three legs

The Alolan Whip's victim does not fly in one arc. The native recording decomposes it:

| Leg | Frames | Status | Source |
|---|---|---|---|
| outbound | 8 | `SWING_GAOGAEN_THROWN` (`0x177`) | `_motion_frame_fix` |
| held at the rope | **20** | `SWING_GAOGAEN_ATTACH_ROPE` (`0x178`) | not in `param_special_s` — it belongs to the rope article, so it is the one measured constant in the move |
| return | 8 | `SWING_GAOGAEN_RETURN` (`0x179`) | `_return_motion_frame_fix` |

The throw fires at animation frame 36. The rope catches the victim at `off = -41.55`, not at the
`-24` the port used for rounds — **`-24` is one of three vertical tripwires** on `top` (y 2..12,
radius 2), crossed on the way out and again on the way back. The victim runs (`hash40("run")`)
only on the rebound, grounded, which is native.

And the second `send_event_nodes` goes to **link 3 — the rope, not the victim.** The rope receives
it and moves the victim through `ATTACH_ROPE`. A port that goes `THROWN → RETURN` directly is
exactly the reported "Mario teleports into the ropes, there isn't any animation for him going
there".

## The cargo, and why the carry is eleven statuses

DK's carry is `0x1EA → 0x1EB → 0x1EC/0x1ED` and nine more — a complete alternate movement mode,
not a motion swap on top of `WAIT`/`WALK`. Trying the motion-swap version is why the carry
animation died the moment Pikachu moved.

- `SHOULDER_WALK` has **three** lines, and its `Exec` calls
  `sub_walk_uniq_process_main_common("shoulder_walk_fast", "_middle", "_slow", NIL)` every frame.
  Without it `calc_walk_speed` and `set_walk_motion` never run after frame 0, and with
  `FIGHTER_KINETIC_TYPE_MOTION_LOOP` the animation is what moves him — so a rate that is never set
  means a frame that never advances means zero displacement.
- **There is no `SHOULDER_DASH` or `SHOULDER_RUN`.** Carrying loses dash and run, which is native.
  And `param_lifting_walk` says the opposite of what it sounds like: carrying makes DK walk **1.1×
  faster**; what he loses is the fast options.
- Every one of the eleven passes **`FS_SUCCEEDS_KEEP_ATTACK_ABSOLUTE`**, which is what carries the
  capture across a status change.
- The throw's `pre` keeps the THROW work (`KEEP_FLAG_THROW_INT` holds `TARGET_OBJECT`); passing
  `NONE_INT` wipes it.
- `throw_loop` must exit on **`CancelModule::is_enable_cancel`**, not on `MotionModule::is_end`.
  `throw_f_hi` has FAF 30 against a 49-frame animation, and a brand-new status id is not in the
  engine's throw family, so nothing reads the FAF for it — nineteen invented frames of end lag.
- The carry exit writes the throw's motion name into `CATCH_WAIT_WORK_INT_MOTION_KIND`; the throw's
  init reads it back and maps it to `thrown_f_donkey_{f,b,hi,lw}`. Every fighter already ships
  those animations, so it costs no assets.
- DK's throw loop switches kinetics on the situation, keyed on `globalTable[0x17]` vs `[0x16]` —
  **previous** versus current, so it fires on the frame the situation flips — and the whole
  dispatch sits behind `!StatusModule::is_changing`. Running it every frame off the current
  situation instead is what produced the landing slide.
- `change_status`'s second argument is **not** always `false`. DK passes `true` for every
  transition the player asked for that frame (a stick crossing a threshold, a button) and `false`
  for the ones the world forces (a situation change, a motion end). All twenty values matter.
- DK drops a held item when **taking a hit** while carrying, not when starting the carry — the
  calls are in `check_dmg`, which is the one status line this project still cannot register (see
  [open-questions.md](open-questions.md)).

## Revenge, the one that was kept

Of the three hard moves, Revenge is the one ADR 0006 did **not** scrap, because it is the module
that got the state question right first: *state is per-entry statics, not work variables.* Its
mechanics, for reference:

- Initial boost `0.075 × d + 1.5`; stacking `m + (d × 0.075 + 0.5) × (1 - 0.25 × (m - 1))`, capped
  at 3× at 20%; Incineroar takes **0.4×** incoming damage during the window.
- The counter fire "deals minimal damage with set knockback" and is **not** boosted. Spending the
  boost on the counter's own fire was implemented twice and is the single most-repeated bug in the
  project's history — `AttackModule::is_attack_occur` is sticky for the whole move, so the read
  must be a rising edge.
- The window arms `DAMAGE_NO_REACTION_MODE_ALWAYS` (the same machinery as Little Mac's f-smash
  armor), so the hit lands, produces no knockback, and the counter fires after a freeze of
  `floor((absorbed × 0.65 + 6) × 1.5)` frames — Ultimate's published hitlag formula times
  Incineroar's own `special_lw_hit_stop_mul`.
- The retaliation is its **own status** (`SPECIAL_LW_HIT`). Requesting `SPECIAL_LW` from inside
  `SPECIAL_LW` is dropped by the engine, so no `main` ran and the hitbox never existed.
- `End` must revert all three absorb calls. `SpecialLw_end` in the decomp is three unconditional
  reverts; a port that does nothing there leaves halved damage and total flinch immunity armed for
  the rest of the stock, invisibly.
- Revenge is **cancellable at its FAF** — the first statement of its `main_loop`. And the stance
  plays `special_lw_start`, not `special_lw`; mixing those up made `PHASE` stick at
  `PHASE_COUNTER` and produced both "fire on command" and "autocancel" at once.
- Per-category knockback ceilings: neutral and smash 1.2×, tilts, dash, and aerials 1.15×, throws
  1.1×; pummel, floor, and edge attacks are exempt.
