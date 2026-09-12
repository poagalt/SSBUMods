# Donkey Kong's cargo carry, read from the source

The whip's rewrite is done (ADR 0006, `src/specials/whip/`). This is the same groundwork for the
other move that decision scrapped, written down before the rewrite so the rewrite is a
transcription and not another round of inference.

## The finding that matters: the carry is not in `pre`/`main`/`end`

`tools/disasm --statusmap` on `lua2cpp_donkey.nro` recovers 178 status functions. Eleven statuses
make up the carry — `SHOULDER_START`, `_WAIT`, `_WALK`, `_TURN`, `_JUMP_SQUAT`, `_JUMP_SQUAT_B`,
`_JUMP`, `_FALL`, `_LANDING`, `_PASS`, `_END` — plus the four cargo throws `THROW_F_F/_B/_HI/_LW`.

Tracing all 45 of their `STATUS_PRE`/`STATUS_MAIN`/`STATUS_END` entries produces almost nothing.
Most are stubs; `SHOULDER_JUMP_SQUAT`'s main is at `0x710001eb00` and its end at `0x710001eb10`,
sixteen bytes apart. That is not a gap in the dump. The carry statuses are thin wrappers over
`L2CFighterCommon`'s ordinary walk, turn, jump and land logic, because carrying someone is
ordinary movement with a passenger.

**The carry itself lives in a different set of status hooks**, which the port does not register at
all:

| hook | address | what it does |
| --- | --- | --- |
| `SHOULDER_START` `INIT_STATUS` | `0x71000083e0` | `capture(MA_MSC_SET_IGNORE_CATCHING)`, then `set_invincible_frame_global(get_param_int("common", "throw_invincible_frame"))` |
| `SHOULDER_WALK` `INIT_STATUS` | `0x7100008650` | the walk-while-carrying setup |
| `SHOULDER_WALK` `EXEC_STATUS` | `0x7100006170` | the per-frame walk drive |
| `THROW_F_*` `INIT_STATUS` | `0x71000087a0` | shared by all four throws: picks the motion pair, `send_event_nodes_throw`, then stores the node id in `FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT` |
| all eleven `SHOULDER_*` `EXIT_STATUS` | `0x7100004df0` | shared: writes the chosen throw motion into `FIGHTER_STATUS_CATCH_WAIT_WORK_INT_MOTION_KIND`, then `capture(MA_MSC_SET_IGNORE_CATCHING)` |

`src/specials/shoulder/` and `src/specials/cargo/` between them register **eleven `Main` and ten
`Pre` hooks and no `Init`, `Exec` or `Exit` at all.** The content is not the problem —
`shoulder::enter_carry` and `leave_carry` reproduce the init and exit bodies call for call, down to
reading `throw_invincible_frame` off `"common"` rather than off a fighter object. They are called
from the wrong place.

### Why that is a real bug and not a detail

`EXIT_STATUS` runs on **every** exit from the status, including the ones the port does not cause:
taking a hit, being grabbed, dying, the stage killing you. `leave_carry` called from a `Main` only
covers transitions the port itself drives. Every other way out of a carry leaves the victim
attached with the motion-kind never written — which is the shape of "the victim is stuck to me" and
of "the throw played the wrong animation".

`INIT_STATUS` runs before `pre`. Setup the engine expects to be in place by then is, in the port,
happening a status function later.

**smashline supports all three.** `create_agent.rs` lists `Init`, `Exec`, `ExecStop`, `Exit`,
`Post`, `MapCorrection`, `FixCamera`, `CheckDamage`, `CheckAttack` and more alongside
`Pre`/`Main`/`End`. Nothing had to be worked around; the hooks were simply never used.

## What the recording already confirmed

`docs/the-recording.md` caught the throw side of this from the outside, and the source now explains
every line of it:

```
DONKEY0  LinkModule::send_event_nodes(link=0, hash40("shoulder"), 0)          <- the lift
DONKEY0  LinkModule::send_event_nodes_throw(hash40("shoulder_throw"),
             hash40("thrown_f_donkey_hi"), true, 0x1100000b, 0x1100000c, 0x1000007)
DONKEY0  CatchModule::catch_cut(false, false)                                 <- the release
```

The three work ids in that call are `FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_GROUP`,
`_TARGET_HIT_NO` and `FIGHTER_STATUS_THROW_WORK_FLOAT_MOTION_RATE`, and `THROW_F`'s `INIT_STATUS`
is where they come from. The four-way motion mapping `shoulder::thrown_motion` already implements
is the one in that function. That part of the port was right and is worth keeping.

## The one thing DK does that Incineroar does not

Donkey Kong ends a carry with `CatchModule::catch_cut` on himself — four instructions,
`this->[0x93] = 1`, a request the engine acts on. Incineroar's whip instead tears down the LINK
(`0x71000206c0`, transcribed as `whip::whip_release`). **Two different release protocols for two
different moves**, and the port spent several rounds applying a third that neither uses
(`CaptureModule::capture_cut`, which `docs/the-recording.md` shows is called by nobody).

So the carry rewrite must use `catch_cut`, not the whip's `whip_release`. They are not
interchangeable and the only reason to think they might be is that both moves hold someone.

## Open, and deliberately not guessed

- `SHOULDER_WALK`'s `EXEC_STATUS` at `0x7100006170` is the per-frame carry drive and has not been
  read yet. It is the next thing to decompile.
- `throw_invincible_frame` was reported reading 0 in a playtest log. The port's read matches the
  source exactly -- same object (`"common"`), same param, same call -- so if it still reads 0 the
  cause is when it is read, not how, and moving it into `INIT_STATUS` is the first thing to retest.
- The `SUPER_LIFT_*` family (ten more statuses, sharing `EXIT_STATUS` `0x71000049f0`) is the up-throw
  super lift, not the cargo. Out of scope for this moveset, listed so it is not mistaken for part of
  the carry later.

## The victim's half, read from `lua2cpp_common`

Added after the whip's victim side turned out to be the whole of its remaining bug. The same
recipe applies here, and the carry had none of it either.

`L2CFighterCommon::status_CaptureWait` (common `0x71000b1150`):

```
WorkModule::set_int(SITUATION_KIND_NONE, FIGHTER_STATUS_CAPTURE_WAIT_WORK_INT_SITUATION)
status_CaptureWait_Loop
flip_have_item_capture
ControlModule::start_clatter_motion_rate          <- the carry clatter, from the CAPTOR's params
sub_shift_status_main
```

`CaptureWaitCommon` (`0x71000b1980`), every frame:

```
if is_flag(FIGHTER_STATUS_CAPTURE_PULLED_WORK_FLAG_JUMP)  -> change_status(CAPTURE_JUMP)
if is_enable_transition_term(..._TRANSITION_TERM_ID_CAPTURE_CUT) -> change_status(CAPTURE_CUT)
```

So the victim mashes out upward into `CAPTURE_JUMP` or escapes into `CAPTURE_CUT` when the engine
enables that transition term. None of it is the captor's business, and none of it runs unless the
victim is actually in `FIGHTER_STATUS_KIND_CAPTURE_WAIT`.

**FOUR THINGS MUST BE TRUE before any of that happens**, and `enter_carry` had zero of them:

1. the capture recorded with us as master — `CaptureModule::capture`
2. the link on **both** ends — `LinkModule::link` twice, because `Link(this, no, id)` writes only
   the caller's slot (Ghidra, main + 0x47f120)
3. the victim forced into `CAPTURE_WAIT` — an unforced `_from_script` request is declined
4. the captor's params answerable — `src/params`, already true

The log said so plainly and it was read as a symptom for several rounds:

```
SHOULDER p0 carry start: NOTHING on LINK_NO_CAPTURE, so the native event is not sent
```

`docs/the-recording.md` is the acceptance test: the real carry logs
`mario1 CaptureModule::motion(hash40("capture_pulled"))` and then `capture_wait` every frame,
both against the VICTIM. Those are not calls to copy — they are what the victim's own status does
once it is running.

## `throw_invincible_frame` reads 0, and the number is not in any file here

DK's `SHOULDER_START` `INIT_STATUS` reads it, and the raw hashes confirm the annotation rather than
relying on it: `0x6e5ec7051` has a length byte of 6 (`common`) and `0x1687ff50b5` a length byte of
0x16 = 22 (`throw_invincible_frame`). The port makes the identical call and gets 0.

It is not in `pikachu/param/vl.prc`, not in `donkey/param/vl.prc`, not among the 808 params of
`fighter/common/param/common.prc`, and not in `vendor/param-labels`. So the parameter is real and
lives somewhere this project has not extracted. **Left at 0 rather than invented**: the cost is
that the port's lift has no invincibility, which is a stated behavioural gap and not a guess. The
way to settle it is to hook `get_param_int` and log what the REAL Donkey Kong receives during a
real cargo lift — a measurement, one playtest, no inference.
