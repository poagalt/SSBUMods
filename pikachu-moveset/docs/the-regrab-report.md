# The regrab lockout: what it was, and what is left

Asked for after the round of 2026-09-13: *"give me a full report on the regrab bugs and what's
needed to progress on them. This needs to be investigated much more thoroughly. Consider pulling
more game data or a different angle of attack. Also investigate how native the current
implementations are."*

The different angle turned out to be the one that mattered, and it was not more game data. It was
reading the log the port already writes in the other direction: **a blocked grab and a whiffed
grab are different shapes, and every previous round had read the blocked one as a whiff.**

---

## 1. A blocked grab is not a whiff

Pikachu's `catch` motion is 50 frames long and his catch collision is on frames 6-7. So the
duration of the CATCH status says, on its own, whether the grab connected:

| digest | count in the logs | what it is |
|---|---|---|
| `CATCH 7f -> CATCH_PULL` | many | **connected** -- the collision landed and the grab started |
| `CATCH 51f -> WAIT` | 30 | **whiffed** -- the whole animation ran, nothing was in range |
| `CATCH 8f -> WAIT` | 14 | **connected, then failed to convert** |

The third row is the reported bug. It ends on the frame a *successful* grab ends on, so the
collision found the target; the engine then declined to start the grab. Nothing about it is a
range problem, and three rounds were spent treating it as one -- checking Pikachu's reach, his
`catch` ACMD, the grab hitbox sizes.

What can make the engine decline a landed catch is the **target**: a fighter the engine still
believes is captured, or one left with `ignore_catching`, cannot be caught by anybody.

## 2. The whip never let go -- not once, ever

`whip_release` is the whip's teardown: two unlinks, a CAPTURE_CUT safety net for the victim's
model constraint, and `victim::drop_capture`, which clears the engine's record of the catch off
the victim. It is registered on `status_end` for all six whip statuses, because that is where
Incineroar has it -- `gaogaen/status/SpecialSThrow/` contains `end.c` and no `exit.c`.

It has never run:

```
whip catches logged, pt19 + pt21 + pt22              ~100
"dropped the capture (whip over)"                       0
"dropped the capture (throw over|carry released)"      15   <- the carry, which uses Exit
```

`drop_capture`'s closing diagnostic is unconditional, so zero lines is zero calls. The two
`TRACE pika0 st=0x1f5 LinkModule::unlink(link=0)` lines in pt22 looked like evidence against this
and are not: they are `end_main`'s own unlink, running in 0x1f5 because that is the status whose
Main they are, and they appear exactly twice against the two `ENGINE caught` cycles.

**smashline's `End` line does not reach a status id past the fighter's own count.** The carry's
`Exit` line does, and its 15 releases are the proof. Both lines dispatch from the same engine
function, one after the other -- smashline hooks End at `+0x689c` and Exit at `+0x6950`, and both
offsets fall inside `L2CFighterBase::sub_end_status_msc` (0x7100006730 -> 0x71000069f0) in
`vendor/main/common-nro-symbols.txt` -- so moving the body to `Exit` is the same moment of the
same transition, not a different behaviour.

So: every whip left its victim with the engine's capture record standing, and no grab could start
afterwards. That is the whole of *"after hitting a cargo throw or alolan whip in any capacity
modified pikachu cannot grab the opponent"*, and the release code was sitting there correct and
unreached the entire time.

The pt21 timeline shows it directly:

```
lines 87-1299     11 whips, none released
line  2413        grab BLOCKED (10f)
line  2547        cargo release runs (drop_capture)
line  2548        grab WORKS (7f -> CATCH_PULL)        <- the very next grab
lines 4460-6429   17 whips, none released
line  6561        grab BLOCKED (8f)
```

## 3. Two more end lines were dead the same way

The same registration bug, found by turning the finding into a build gate rather than by
playtesting:

* **both up-B statuses** (`SPECIAL_HI_CLOSE`, `SPECIAL_HI_OPEN`) set
  `JUMP_COUNT = JUMP_COUNT_MAX` in their end line -- G&W's parachute costs you your jumps.
  Transcribed correctly, never reached, so modified Pikachu has been keeping a double jump G&W
  does not have. Nobody had reported it because you have to be looking for it.
* **`failure_end`'s `reverse_lr`** -- Incineroar ends a whiffed Alolan Whip *facing the other way*.
  Pikachu has been shrugging and standing still through every one of the 24 failures in pt22.

`tools/check-statuslines.sh` now scans the registrations instead of the decomp, which is what
catches `SPECIAL_HI_OPEN`: it is the port's own construction standing in for an article ADR 0004
excludes, so it has no source directory to check against, and its end line was just as dead.

## 4. A second, smaller defect: the empty sentinel was player one

`HELD_ID`'s "nobody held" value was `0`, and `id_of_entry` is `entry << 16` -- so `0` is player
one's battle object id. The line that caught it:

```
HOLD p0 dropped the capture (carry released): ... their_capture=true their_st=0x1f8
```

`0x1f8` is SHOULDER_WAIT, one of this port's own **captor** statuses. No victim can be in it.
`boma(HELD_ID[entry])` had resolved to Pikachu himself, so every `their_*` field printed at
release time was read off the wrong fighter -- three rounds of misleading diagnostics, including
the `their_capture=TRUE` that an earlier comment in `victim.rs` built a whole theory on.

It was also a real functional bug for any Pikachu not in slot one: grabbing player one stores id
`0`, which every `== 0` guard reads as "nobody held", so the hold is never ticked, released or
forgotten.

## 5. How native the implementations actually are

Measured against 8070 traced frames of real Incineroar. The native flight is completely
deterministic -- 58 whips, zero variance:

| phase | native | the port before this round | now |
|---|---|---|---|
| CATCHED, victim held at | off +16.48 for 19 frames | +16.48 for 19 frames | exact |
| CATCHED, drag-in | +14.74, +9.14, +1.49 | +14.74 | exact |
| THROWN start | off -7.09 | -7.09 | exact |
| THROWN rate | -11.20/frame (`through_target_speed`) | -11.20/frame | exact |
| **THROWN length** | **4 frames, to off -40.69** | 3 frames, to -29.49 | FIXED |
| **rope stretch** | **-7.84 more (-6.60, then a 1.24 spring tail)** | none -- frozen in place | FIXED |
| rope hold | 19-20 captor animation frames | 20 (`ROPE_HOLD_FRAMES`) | exact |
| RETURN rate | +2.80/frame (`through_target_return_speed`) | +2.80/frame | exact |
| failure turn-around | `reverse_lr` | never ran | FIXED |
| release | `status_end` body | never ran | FIXED |

Two conclusions worth stating plainly:

* **Everything the engine drives is already exact.** All three speeds, the catch hold, the
  drag-in, the rope hold duration. The port's own numbers are the source's numbers.
* **Everything the port has to decide for itself was wrong, and in the same direction:** when the
  outbound leg ends. It was ending on `special_s_lariat_offset_x1` (-24), which the docs had
  already recorded is *not* the rope -- -24 falls between the second and third frame of a
  four-frame leg. So the victim attached one frame early at -29.49 and then froze for the whole
  20-frame hold, which is the flat `off=-29.49` repeated ten times in pt22. Native reaches -48.53.
  **That is "4 tiles in the training room instead of 5", and it is also why the bounce "seems too
  long": the hold length was always right, but nothing moved during it.**

The rope's position is now a measured constant beside `ROPE_HOLD_FRAMES`, with the same
justification -- the ropes belong to the ROPE article that ADR 0004 excludes, so they are not in
`param_special_s`, and 58 identical native whips are the best evidence available. `-40.69` is
exactly `-7.09 - 3 * 11.20`, so it is really "four frames of the game's own speed" written as the
distance the trigger needs.

## 6. What is still open

**The cargo's half of the lockout is narrowed, not closed.** The carry's `Exit` fires and
`drop_capture` runs, but in pt21 it printed `their_capture=true` on a real victim id after
`capture_cut(vboma, true, false, false)` had been called. Either the gate above it
(`catch_object_id` still naming us) did not match, or `is_capture` does not clear until the
victim's CAPTURE_CUT status *ends* a few frames later -- in which case that line is harmless and
the two blocked grabs after a cargo release in pt21 are whip residue from the 11 unreleased whips
earlier in the same session, which is the parsimonious reading.

Nothing in the logs distinguishes those, so the port now measures it instead of arguing about it.
`probe::catch_start` fires on the frame Pikachu enters CATCH and prints, for every live fighter:

```
CATCH p0 starting: mine is_catch=.. ignore_catching=.. is_capture=..
  | 0x10000 st=.. is_capture=.. ignore_catching=.. captured_by=0x.. clatter=..
```

That is the complete set of things that can decline a landed catch, on the frame it is declined.
One playtest with a grab attempt after a cargo throw answers it.

Supporting change: the victim-side capture statuses are now named in the log from the game's own
LuaConsts rather than printed as raw hex. `their_st=0x44` was unreadable, and the `0x44 IS
CAPTURE_CUT` assertion in `victim.rs` is not proven by anything local -- so the log says the name
now and there is nothing left to remember or to get wrong.

**One fidelity gap, recorded rather than guessed at.** Native spends the stretch inside
ATTACH_ROPE -- THROWN is four frames, the status changes, then the victim slides the last 7.84.
This port spends it inside THROWN and enters ATTACH_ROPE at the far end, because the engine's
ATTACH_ROPE PINS the victim for us: with no rope article to read a parent position from, the
measured result is the flat `off=-29.49` for the whole hold. So the choice was between the right
position in the wrong status and the right status with no movement, and position is the half that
can be measured. The victim plays THROWN's animation for about five frames where Incineroar's
plays ATTACH_ROPE's. Driving the slide by hand would get both and is deliberately not done yet --
nothing establishes whether the engine re-pins the victim every frame in that status, and a nudge
that loses that fight every frame is the kind of unmeasured fix ADR 0005 exists to keep out. One
playtest observation of how the bounce reads decides it.

**Still unverified by playtest** -- this round's changes are deployed and untested:

1. that the whip's release now runs, and which line the engine calls -- the log will say
   `status_end body reached via the Exit line` (or `End`);
2. that the grab works after a whip and after a cargo throw;
3. that the whip now reaches 5 tiles and the bounce reads right;
4. that Pikachu turns around after a whiffed whip;
5. that the up-B now costs a jump.

**Known remaining non-exactness**, unchanged and previously recorded: Palutena's n-air, 18.53
world units off, because her hitboxes ride a `Stick` bone Pikachu does not have at runtime and she
animates it across the 19 frames the hitboxes are live. No static anchor can follow it; the fix is
to ship the real weapon objects, which is the user's own suggestion and is future work.

## 7. The alternating whip was the same bug, one layer up

*"After one hit alolan whip the next attempted alolan whip stops short of the opponent no matter
what and displays the shrug animation instead of grabbing them, and if I try alolan whip again
after that it works."*

`find_caught_victim` adopts a victim when `CaptureModule::is_capture(them)` and
`catch_object_id(them)` names Pikachu. That is a perfectly good description of a catch the engine
just made -- and it is also, word for word, the description of the record an unreleased whip
leaves behind. So the whip after a whip adopted a victim who was standing in an ordinary status
and had never been caught, and the move collapsed in one frame:

```
WHIP p0 ENGINE caught 0x10000 -- adopting, no port-side claim
VICTIM p0 whip: status=0x1f captured=true master=0x0
MOVE p0 side-B whip: SPECIAL_S 17f -> RAT_SPECIAL_S_THROW 1f -> ..._END 45f -> WAIT
```

Five times across pt21 and pt22. `0x1f` is neither a whip status nor a capture status. And "if I
try again after that it works" is `tick_hold`'s ceiling finally letting go about 170 frames later,
which is roughly how long it takes to try a third time -- hence alternation rather than a
permanent lock.

The root cause was the dead `End` line, so fixing the release fixes this too. But a test that
cannot tell a fresh catch from a leaked record should not be the one deciding, so it now has to
agree with the victim's status as well, and a record that fails that check is logged by name
rather than silently adopted. If a leaked capture ever reaches there again, that line is where it
will be visible first.

---

# Round two (pt23): the `End` line was innocent, and the real guard was invented

The previous section concluded that smashline's `End` line does not reach a new status. **That was
wrong**, and the same playtest that disproved it also proved the fix worked for a different
reason: the up-B now costs its double jump, which only happens if an end line ran.

```
status_end body reached via the End line, leaving 0x1f2   26
status_end body reached via the End line, leaving 0x1f3   29
status_end body reached via the End line, leaving 0x1f4   65
status_end body reached via the End line, leaving 0x1f6    9
```

129 calls. `End` fires. What never ran was still `whip_release`, and `probe::catch_start` -- added
in the last round for exactly this -- said why on its first outing:

```
CATCH p0 starting: mine is_catch=false ignore_catching=false is_capture=TRUE
  | 0x10000 st=WAIT(0x0) is_capture=true ignore_catching=false captured_by=0x0 clatter=85
```

**`CaptureModule::is_capture` is true on the CAPTOR, not just the capturee.** Ten of the twelve
grab attempts in pt23 read it set on Pikachu himself. And the first line of `whip_release` was:

```rust
if CaptureModule::is_capture(boma) { return false; }
```

justified in a comment as "a Pikachu who got grabbed mid-whip must not tear down the grab holding
him". A real concern, the wrong predicate, and **not in the source at all** -- so the guard fired
on every single whip, at precisely the moment the release was needed, and returned before
`drop_capture` could clear the engine's record off the victim. Mario is sitting in `WAIT` in that
line with `captured_by=0x0`, which is Pikachu.

The same probe shows the other side of it, and it is unambiguous:

```
CATCH p0 starting: mine is_capture=false | 0x10000 st=WAIT is_capture=false captured_by=0x50000000
MOVE p0 grab: CATCH(0x37) 7f -> CATCH_PULL(0x38)        <- the one grab that worked all session
```

## What the source actually does

Reading `SpecialSThrow_end` (`0x7100004ba0`) properly, rather than the shared helper it inlines,
turned up four separate divergences beyond the guard:

| | source | the port had |
|---|---|---|
| the guard | `next_status` not in {SHOULDER, LARIAT, FAILURE, END} | `is_capture(boma)` |
| `get_parent_id` | `(link, **true**)` | `(link, false)` |
| parent-event gate | `!is_flag(THROW_ROPE_END)` | `is_flag(FLAG_HIT)` |
| parent-event hash | `0x1465ed7a5c` | `0x18f836a168` -- a value that appears nowhere in the decomp, nowhere in `the-recording.md` and nowhere else in this repo |
| `is_linked` block | a **sibling** of the `is_link` block | nested inside it |

The guard is the same shape as DK's shoulder `exit.c`: do not let go of the victim when the status
you are leaving for is another part of the same move. Nothing about being captured.

And `SpecialSFailure_end` turns Incineroar around only when `globalTable[0xb] == DOWN_SPOT` -- when
the whiff puts him on the floor -- not on every grounded whiff. The port read it as a situation
test. **The playtest reported this before the decomp did:** *"Pikachu now behaves correctly after a
missed whip, no turn around."* No turn around is the correct behaviour.

## The distance, second pass

pt23 measured the previous round's assumption and it does not hold:

```
port    f38 -18.29   f40 -29.49   f42 -40.69   f44 -51.89     vst=0x177
        f46 .. f64   -51.89 for every one of the twenty hold frames   vst=0x178
native  0x177 four steps to -40.69, then 0x178 slides to -48.53
```

Five full 11.20 steps where native takes four, then dead still. The theory was that passing
`SPECIAL_S_FLOAT_ROPE_X` would make the engine drop the victim to `_speed_over_rope` and that slow
leg would be the stretch. Both hooks are being read -- `rope_x` and `GENERATED_ROPE` are in the
param log -- and the engine does not act on either in time. At 11.20 a frame there is no landing
anywhere near the give: the step that crosses the line overshoots by a whole 11.20. That is "still
sometimes goes further than 5 tiles".

So the attach moved back to the rope line, which is native's timing *and* native's position, and
the give is now paid out by this port -- one `special_s_through_target_speed_over_rope` per frame
until the measured 7.84 is spent -- inside ATTACH_ROPE, which is native's status as well. The
"known gap" recorded last round is closed rather than documented: the move is forced into that
status by this file, so driving it from this file is the same ownership `take_captive` already
takes when it calls `CaptureModule::capture` by hand.

## Method note

Two rounds in a row the wrong conclusion came from the same mistake: inferring a mechanism from
the absence of a log line instead of measuring the mechanism. "No release logged, and the carry's
Exit does log, therefore End is dead" was a plausible story that fit every fact available and was
still wrong. What settled it was a probe that printed the actual engine state at the actual
moment -- and the honest read of that is that the probe should have come first, before the theory.

---

# Round three (pt24): the whip is fixed, and the cargo was breaking its own throw

The `is_capture` guard was the whip's whole problem. pt24:

```
dropped the capture (whip over)   94   MY_is_capture=false their_capture=false
```

Ninety-four releases where there had been zero, all clean on both sides, and the playtest
confirms it: *"now I can grab the opponent after an alolan whip lands."*

## The cargo is a different bug with the same shape

The cargo's releases did NOT come out clean:

```
dropped the capture (throw over)  MY_is_capture=true their_capture=true their_st=CAPTURE_CUT(0x44)
```

and the probe agrees about the consequence -- clean state grabs, dirty state does not:

```
PROBE mine_cap=false mario cap=false by=0x50000000   ->  CATCH 7f -> CATCH_PULL     works
PROBE mine_cap=true  mario cap=true  by=0x0          ->  CATCH 13f -> WAIT          blocked
```

The cause is three lines apart in the log:

```
SHOULDER p0 throw init: ... victim plays thrown_f_donkey_hi, target=0x10000
about to force status 0x44 on victim from stale capture record (they are in 0x49)
HOLD p0 dropped the capture (throw over): MY_is_capture=true their_capture=true their_st=CAPTURE_CUT
```

The victim is in `0x49` -- **being thrown**, one frame after the port told them to play
`thrown_f_donkey_hi` -- and `drop_capture` yanks them into CAPTURE_CUT. Its test is "the engine
still names us as their captor", which is true of a live throw exactly as it is of a leaked
record, so it cut a throw that was working. CAPTURE_CUT from mid-throw completes nothing and both
sides stay flagged.

**This is the third instance of one mistake**, and it is worth naming as a class:

| predicate | true for | also true for |
|---|---|---|
| `whip_release`'s `is_capture(boma)` | a Pikachu who got grabbed | a Pikachu who is holding someone |
| `find_caught_victim`'s adopt test | a catch the engine just made | a record an unreleased whip leaked |
| `drop_capture`'s victim block | a stale capture | **a throw in progress** |

Each one is a state predicate being asked a question about *history* that it cannot answer.

## What the source does instead

`ThrowFF_exit` in DK's decomp is one line:

```c
app::lua_bind::CatchModule__catch_cut_impl(this->moduleAccessor, false, false);
```

No `capture_cut`, no forced CAPTURE_CUT, no unlink. Neither source fighter calls `capture_cut`
anywhere -- the engine capture tracer recorded zero across a whole session of real DK and real
Incineroar -- because their captures are the ENGINE's and its release path does this internally.

So `drop_capture` now takes `ours_by_hand`. The whip passes `true`: `take_captive` calls
`CaptureModule::capture` itself, so the port owns that record and has to undo it. The cargo passes
`false`: it reaches the carry out of an ordinary grab, the capture is the engine's, and the
engine's own teardown is the whole of it.

## Distance, third pass

The stretch drive worked exactly -- 7.84 paid out, -51.89 to -59.73. The attach did not move to
the rope line as intended, so the give went on top of an already-late attach. That is the 6 tiles.

```
reached the ropes after 4f at off=-51.89 (rope_x=-51.58 their_x=-62.78 by_place=true)
```

`their_x` is one full `_speed` step past `rope_x`, so the frame before it the victim stood at
-51.58 -- `rope_x` to every digit printed. `(pos - rope_x) * lr <= 0.0` is an exact-equality test
between two floats reached by different arithmetic: the engine integrating `start + 3 * 11.2`,
against `captor_x + ROPE_OFFSET_X` latched at the throw. It came out a hair positive on all fifty
whips, consistently, because the same arithmetic gives the same residue.

The victim moves in whole `_speed` steps, so "has reached the line" cannot be resolved finer than
one step. The test is now which step lands *nearest* the line, with half a step as the midpoint
between the two candidates -- 0.0 away against 11.2 away. No tolerance constant; the game's own
speed is the whole of it. That puts the attach at off -40.69 and the extreme at -48.53, which is
native.

---

# Round four (pt25): the cargo captures by hand too, and the cut has to wait

The whip is done. The distance is right -- *"Alolan whip now goes the correct distance"* -- and
the release is clean on every one of pt25's whips.

Last round's cargo fix was built on a premise pt25 disproves, in the cargo's own trace:

```
TRACE mario1 st=0x44 mot=0xb319a8b44 CaptureModule::capture(captor=0x0, 0xffffffff, true, 0)
SHOULDER p0 established capture on 0x10000: is_capture=true their_is_link=true their_st=0x42
```

Mario is at `0x44` -- CAPTURE_CUT, the ordinary grab already finished -- and **the carry
re-captures him itself**. So `ours_by_hand=false` was wrong: the port owns the cargo's record
exactly as it owns the whip's, and passing `false` meant nothing undid it at all. Every `throw
over` in pt25 reads `MY_is_capture=true`, and the probe finds both fighters still flagged at the
next grab.

So the cut is necessary *and* the moment was wrong, both at once, and the two are not
distinguishable by any state predicate — which is the fourth instance of that class in this bug.
What separates them is **time**, so the cut now waits instead of testing: `drop_capture` and
`cut_capture` record the victim's status and arm; `tick_deferred_cut` fires
`capture_cut(vboma, true, ..)` on the first frame that status is something else, which is the
throw having finished with them. No status id has to be identified — only that it changed. A
deadline of `2 × MAX_HOLD_FRAMES` backstops it so a victim who never moves on cannot hold the
capture open, which would be this very bug reintroduced by its own fix.

The mash-out path had the same hole from the other side: `cut_capture` requested CAPTURE_CUT on
the victim and never undid `CaptureModule::capture`. *"Still can't grab mario after a cargo throw
mash out or successful throw"* is one sentence because it was one omission. Both arm now.

---

# Round five (pt28): there is no release call, so stop needing one

The ladder ran all four candidates, one per frame, and every one failed with `their_captured_by`
stuck at `0x0`:

```
release candidate 0 capture_cut(v, true, false, false):     cleared=false
release candidate 1 CatchModule::set_catch(ours, INVALID):  cleared=false
release candidate 2 capture_cut(v, true, true, true):       cleared=false
release candidate 3 capture(v, INVALID, ..):                cleared=false
                                              (INVALID is 0x50000000)
```

**A capture this port makes with `CaptureModule::capture` cannot be undone by any call the port
knows of.** That is the finding, and it is a better one than another failed guess: it says the
fix is not a missing function call, it is not making the capture at all.

## Why the port makes one, and why DK does not

Native DK, from the playtest where the user grabbed as him:

```
DONKEY0 status CATCH_DASH(0x39) -> 0x3a -> 490(0x1ea)  SHOULDER_START
DONKEY0 st=0x3a  CatchModule::set_catch(0x50000000)
DONKEY0 st=0x1ea LinkModule::send_event_nodes(link=0, event=0x8af9b3d1d, 0)
mario1  st=0xc5  ControlModule::start_clatter(139, 0, 10, ...)
```

He goes from a CATCH status **straight into SHOULDER_START**, never enters `THROW`, and never
calls `CaptureModule::capture`. The "shoulder" link event alone carries the victim across --
because the engine's own grab is still intact when it arrives.

This port, three times in the same log:

```
pika0  status CATCH_WAIT(0x3c) -> THROW(0x40)
pika0  st=0x40  CatchModule::catch_cut(false, false)      <- the grab is destroyed
mario1 st=0x44  CaptureModule::capture(captor=0x0, ...)   <- enter_carry rebuilds it
pika0  status THROW(0x40) -> 503(0x1f7)
```

`pikachu_throw_main` intercepts on THROW's **Main**, and its comment claimed "Pikachu's own throw
never runs, so nothing is released". `catch_cut` happens on the way **in**, before Main is
reached. So the engine's grab was already gone, `enter_carry`'s `if !LinkModule::is_linked(..)`
was true, and it built the replacement capture that nothing can undo.

Every cargo symptom of the last five rounds is downstream of that one frame.

## The fix

`pikachu_catch_wait_main` takes the forward throw in `CATCH_WAIT`, on the engine's own
`FIGHTER_PAD_CMD_CAT2_FLAG_THROW_F`, and goes straight to SHOULDER_START. `THROW` is never
entered, `catch_cut` never fires, the engine's grab survives into the carry, `enter_carry`'s guard
is false, and there is no hand-made capture to leak.

Back, up and down throws are untouched -- they test a different flag and reach Pikachu's own
`THROW` exactly as before. `pikachu_throw_main` stays as the fallback, so if the flag is not
raised in that status the behaviour is what it is today rather than a broken grab.

## And the status names were lying

`probe::status_name` had `56 => "CATCH_PULL", 57 => "CATCH_WAIT"` as hardcoded literals, and four
rounds of logs were read through them. The native DK trace is what exposed it: he reaches the
carry from `0x39` and our Pikachu reaches `THROW` from `0x3c`, which the table printed as the
same name. The whole catch family now resolves from `FIGHTER_STATUS_KIND_*` instead. A status
number this project reasons about should never be a number somebody remembered.
