# ADR 0006 — Transcribe the source fighter's status tree; do not reimplement it

Status: accepted, 2026-09-12.

## The decision

Alolan Whip and the cargo carry are **scrapped and rewritten** as line-by-line transcriptions of
the source fighter's decompiled status functions. Revenge is **kept**.

A ported move's status functions must be written by reading the decompiled C of the corresponding
source function and transcribing it, with the diff against that source checkable. They may not be
written from a description of what the move does.

## Why the existing two are scrapped

Seven playtest rounds, and each one fixed something real and left the move broken a new way. That
is not bad luck; it is what reimplementation from partial knowledge produces. The clearest example
is the cargo, whose entire entry premise was wrong for three rounds behind a comment that was
confidently reasoned and false:

> The cargo grab is DK's throw_f. Once its motion finishes, the victim is still caught
> (ATTACK_ABS of CATCH kind, not THROW kind), so this is the moment the carry begins.

True of Donkey Kong, whose `throw_f` IS the lift. False of Pikachu, whose `throw_f` is a real
forward throw — so the carry began on a fighter who had already been thrown and released, every
single time. `crate::victim::trace` reported `status=0x44` (CAPTURE_CUT) on the first frame of the
carry for three rounds and it was read as a symptom each time.

No amount of further patching reaches that, because the bug is not in any of the carry statuses.
It is in the decision about where the carry starts, which was never checked against the source.

## Why Revenge is kept

It works. Its four bugs were found and fixed, all sixteen of its params are verified against
`gaogaen/param_special_lw` by `tools/check-srcparams.sh`, and the playtest reports have been about
the whip and the carry for many rounds. Scrapping working code to be consistent is cost without
benefit.

## What changed to make transcription possible

This decision would have been wrong a week ago, because the source was not readable. Four things
arrived:

- **A decompiler.** Ghidra 12.1.3 and a portable JDK in `vendor/tools`, driven by
  `tools/decomp.sh`. Any function in `main` or any NRO decompiles to C on demand.
- **`tools/disasm --statusmap`.** A fighter NRO exports exactly one symbol for its status scripts
  and every status function is an unnamed static. The registration run recovers all of them:
  98 for Incineroar, 178 for Donkey Kong, each with its status kind and line resolved. So
  "Incineroar's SpecialSThrow main" is now an address, not a guess.
- **`vendor/ssbu-status/lua_dict.txt`**, so every `ldr wN, [xM, #off]` in the decompiled output
  reads as a named constant.
- **The recording.** `docs/the-recording.md` is 533 traced engine calls from the real fighters. It
  is the acceptance test: the port's own traced calls must match it.

## The rule this makes explicit, which the old code lacked

**A port may not store its state in another fighter's work ids.** Incineroar's statuses use
instance-work ids past the end of Pikachu's arrays — floats `0x50`, `0x51`, `0x52` against his
`FIGHTER_INSTANCE_WORK_ID_FLOAT_TERM` of `0x4C`, and flag `0xEA` against his `0xE8`. A literal
transcription therefore cannot use `WorkModule` for those, and must route them to storage the
plugin owns, in one place, named after the constant it stands in for.

This is the decision that was never made, and its absence is why the old code drifted: each round
picked a different substitute for the same missing state — a static array here, a recomputed
geometry there, a frame counter somewhere else — and none of them agreed with the source.

## What transcription does not mean

It does not mean calling Incineroar's functions directly. That was considered: his status
functions are real code in a loaded NRO and `L2CFighterGaogaen` extends `L2CFighterCommon`, so the
layout is broadly compatible. It fails on the same wall as the work ids — his code reads and
writes gaogaen instance work that Pikachu's arrays cannot hold — and on the article, and it would
put out-of-range writes inside native code where nothing can guard them. Transcription with an
explicit state shim is the faithful version that is also safe.

## How it will be checked

- `tools/check-nativecalls.sh` already lists every engine call the source status makes that the
  port does not. For a transcribed status that list should be empty except for calls deferred by
  design, and that is a much stronger statement than it was for reimplemented code.
- The recording. Run the port, read its `[rat] TRACE` lines, diff against
  `docs/the-recording.md`. `start_clatter(85, 0, 5, ...)` against `(0, 0, 0, ...)` is the shape of
  the answer.

## Addendum, 2026-09-14: this ADR does not cover how the source ENTERS the tree

Five rounds of the cargo regrab bug came from a status transition this ADR has nothing to say
about, and the gap is worth naming because every ported move that is reached from something other
than a special input has the same one.

Donkey Kong's decomp directory contains `ShoulderStart`, `ShoulderWait`, ..., `ThrowFF` and the
rest of the carry. **It does not contain `CatchWait`.** He does not own that status -- the common
module does -- and this repository has no `lua2cpp_common` decomp, only `donkey`, `gamewatch`,
`gaogaen`, `packun` and `pikachu`. So `CATCH_WAIT -> SHOULDER_START`, the one transition that
decides whether the engine's grab survives into the carry, was never readable.

His tree, as this ADR defines it, STARTS at `ShoulderStart`. The port therefore had to invent the
way in, and nothing in the process marked that as invented. It was inferred from Pikachu's side
instead -- "the forward throw is the lift, so intercept the forward throw" -- and intercepted on
`THROW`'s Main, one frame after the engine's `catch_cut` had already destroyed the grab. Every
cargo symptom for five rounds was downstream of that frame, and the workaround for it
(`enter_carry` rebuilding the capture by hand) produced a record that pt28 proved nothing can
undo.

What settled it was not the decomp. It was the user playing Donkey Kong for one minute while the
capture tracer was armed:

    DONKEY0 status CATCH_DASH(0x39) -> 0x3a -> 490(0x1ea)  SHOULDER_START

He never enters `THROW`.

**So: when a move's first status is reached from a status the source fighter does not own, the
entry is not a transcription and must be written down as an assumption.** The check for it is a
native recording of the source fighter making that transition, not a reading of anything --
because there is nothing to read. `tools/check-statuslines.sh` verifies the lines a status has; it
cannot verify that the port arrives at the first one the way the source does, and no static check
can.
