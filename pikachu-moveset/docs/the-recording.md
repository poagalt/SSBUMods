# What the engine actually does, recorded from the real fighters

Every grab bug in this project has been the same shape: the code that decides "this catch
connected, so put the victim there and me here" is compiled C++ in `main`, keyed on the fighter
kind, and it does not run for a Pikachu. Six rounds of fixes were assembled from static reading —
the fighter NRO, `lua2cpp_common`, `lua_dict`, the param prcs — and each one fixed something real
and left the grab broken a new way. That is what inference looks like when it is nearly right.

`plugin/src/telemetry/enginetrace.rs` hooks the twelve calls that change capture state and logs
each one with its arguments and its caller's fighter kind. One session of the real Donkey Kong and
the real Incineroar produced **533 recorded calls**. This page is that recording, with every hash
resolved. It is ground truth, not interpretation: where this page and any reasoning elsewhere in
the repo disagree, this page wins.

## Donkey Kong's cargo carry, one complete cycle

```
DONKEY0  CatchModule::set_catch(0x50000000)                       clear, at the catch pull
mario1   ControlModule::start_clatter(1788.3, 1, 8, -1, 0, false, false)
mario1   CaptureModule::motion(hash40("capture_pulled"), 0)       x21, every frame
DONKEY0  LinkModule::send_event_nodes(link=0, hash40("shoulder"), 0)          <- the lift
mario1   ControlModule::start_clatter(3274.8, 0, 10, -1, 0, false, false)
mario1   CaptureModule::motion(hash40("capture_wait"), 0)         every frame while carried
DONKEY0  LinkModule::send_event_nodes_throw(hash40("shoulder_throw"),
             hash40("thrown_f_donkey_hi"), true, 0x1100000b, 0x1100000c, 0x1000007)
DONKEY0  CatchModule::catch_cut(false, false)                     <- the release
```

The three cargo throws recorded came out as `thrown_f_donkey_hi`, `_lw` and `_b`, which is exactly
the mapping `shoulder::thrown_motion` already implements, and the three work ids are
FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_GROUP, _TARGET_HIT_NO and
FIGHTER_STATUS_THROW_WORK_FLOAT_MOTION_RATE. That part of the port was right.

**Four of those lines are the VICTIM's own status doing its job** — the two `start_clatter` calls
and both `CaptureModule::motion` loops are logged against `mario1`, not against Donkey Kong. They
are not calls to copy. They are what happens automatically once a fighter is genuinely captured,
and the carry clatter of ~3300 against our victim's 65 is the whole of "mario escapes a brief
moment after being grabbed no matter the percent".

## Incineroar's Alolan Whip, one complete cycle

```
mario1   ControlModule::start_clatter(85, 0, 5, -1, 0, false, false)
GAOGAEN0 LinkModule::send_event_nodes(link=0, hash40("gaogaen_swing_throw"), 0)
mario1   LinkModule::unlink(link=0)
GAOGAEN0 LinkModule::send_event_nodes(link=3, 0x2a5eaa0cb9, 0)    link 3 = the rope article
other0   LinkModule::unlink(link=4)                               the rope, releasing
mario1   LinkModule::unlink(link=0)
```

**`start_clatter(85, 0, 5, ...)` is the independent confirmation of `src/params`.** 85, 0 and 5 are
`special_s_catch_clatter_frame`, `special_s_catch_clatter_damage_mul_frame_add` and
`special_s_catch_clatter_dec_frame` from Incineroar's `param_special_s` — the exact three values
the param provider answers with, arriving in the exact call the disassembly predicted. The
provider is right, and it has never once been reached, because the victim's status that reads it
never runs.

## The three things this proved WRONG in the port

Each had been reasoned to, and each is contradicted by the recording.

| What the port did | What the recording shows |
| --- | --- |
| `CatchModule::set_catch(boma, victim_id)` to claim the victim | `set_catch` is called 7 times and **every one passes 0x50000000** (BATTLE_OBJECT_ID_INVALID). It is a clear, never a claim. |
| `CaptureModule::capture_cut(vboma, false, false, false)` to release | **Neither fighter calls `capture_cut` at all.** Donkey Kong ends every carry with `CatchModule::catch_cut(false, false)` on HIMSELF. |
| Nothing, at the end of a carry | `catch_cut` — and Ghidra shows `CatchCut` is four instructions, `this->[0x93] = 1`. It is a REQUEST, and the engine does the teardown itself, which is why it cannot leave a model constraint behind. |

That last row is the glue. `CaptureCut(this, p2, p3, p4)` gates its entire teardown on `p2`, so
`(false, false, false)` skipped the two vtable calls, 0x3b8 and 0x3c0, that undo the capture. The
victim's model stayed constrained to the captor's `"throw"` bone — Pikachu and Mario welded
together.

## What neither fighter does, which is the remaining problem

**Neither Donkey Kong nor Incineroar ever calls `CaptureModule::capture`.** For them the engine
establishes the capture from the catch collision, in the kind-keyed native code, and none of it
passes through a hookable module function. So there is nothing to copy: `capture` is the only way
in from a plugin, and it demonstrably works (`is_capture=true master=0x0`).

Everything downstream then follows on its own, because the recording shows the interesting work is
the victim's: their status starts the clatter with our params, plays `capture_pulled` and then
`capture_wait` every frame, and unlinks itself when told. The port does not need to reproduce any
of that. It needs the capture to exist and to be released the way Donkey Kong releases it.

## Every hash in the recording

| hash40 | name | where |
| --- | --- | --- |
| `0x8af9b3d1d` | `shoulder` | DK's lift event, to link 0 |
| `0xe9b37bc21` | `shoulder_throw` | DK's cargo throw event |
| `0xe4308c047` | `capture_pulled` | the victim's pull-in pose |
| `0xccfd2e2d2` | `capture_wait` | the victim's carried pose, every frame |
| `0x11474b27b0` | `thrown_f_donkey_f` | cargo throw, forward |
| `0x114026e3a9` | `thrown_f_donkey_b` | cargo throw, back |
| `0x12b3c9e354` | `thrown_f_donkey_hi` | cargo throw, up |
| `0x122daa1b33` | `thrown_f_donkey_lw` | cargo throw, down |
| `0x54f934137` | `throw` | DK's ordinary throw event |
| `0x882d73137` | `thrown_b` | the victim's motion for an ordinary back throw |
| `0x13b7318a5a` | `gaogaen_swing_throw` | Incineroar's throw event, to link 0 |
| `0x2a5eaa0cb9` | unresolved, 42 characters | Incineroar's event to the rope article, link 3 |

The last one is the only hash in the whole recording still unnamed, and it is the one that needs
an article Pikachu does not have. See ADR 0004.

## Reproducing this

1. `ENABLED = true` in `plugin/src/telemetry/enginetrace.rs`, then build, check, deploy.
2. **Fully close and relaunch Smash.** A Skyline plugin is loaded when the GAME LAUNCHES, so
   deploying to a console that is already running it changes nothing. This cost a whole session:
   the log that came back had none of the new instrumentation in it and looked exactly like broken
   code. `tools/analyze.sh` now compares the log's build stamp against the local one and says so
   loudly.
3. Play the SOURCE fighter, not modified Pikachu. For the carry: grab, forward-throw into the
   cargo, walk and turn, then each of the four throws. For the whip: catch, then let it run, then
   an early press, then a late press, then a whiff.
4. `bash tools/analyze.sh`, and read the `[rat] TRACE` lines.
