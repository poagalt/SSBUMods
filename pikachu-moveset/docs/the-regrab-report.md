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
