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
