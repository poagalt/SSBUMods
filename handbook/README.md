# The moveset-mod handbook

Everything this project has learned about making one fighter's move work on another fighter, in
one place. Compiled 2026-09-14 from the whole of `SSBU Pikachu mod moveset theory` (148 turns,
ten compactions), from `BACKLOG.md`, `CONTEXT.md`, `RESTORE.md`, the fourteen pages under
`pikachu-moveset/docs/`, the six ADRs, and from the data this project pulled off the console.

It exists because the knowledge was spread across a chat transcript that no longer fits in a
context window, a 92 KB backlog written newest-section-last, and doc pages named after the round
that produced them. Every fact below was paid for once. The point of this directory is that it
does not get paid for twice.

## Read in this order

| Page | What it answers |
|---|---|
| [the-engine-model.md](the-engine-model.md) | What a move actually *is* to the engine — the five layers, and which layer holds which fact |
| [sources-of-truth.md](sources-of-truth.md) | Where the truth lives, in precedence order, with the command to obtain each |
| [porting-a-move.md](porting-a-move.md) | The procedure, by move class, and how to tell which class you are in |
| [grabs-and-victims.md](grabs-and-victims.md) | Command grabs: the victim's half, capture and release, and the two moves that cost the most |
| [parity.md](parity.md) | What "exact" means per axis, what does not port, and how the checks prove it |
| [traps.md](traps.md) | Every trap this project has paid for, with what it cost |
| [toolbox.md](toolbox.md) | Each tool in `tools/`, as the question it answers |
| [lookup.md](lookup.md) | **One place to look anything up**: `tools/ssbu.py find` over the code index, the move index, constants, hash names, community names and every doc |
| [switch-session.md](switch-session.md) | What still needs the console, and the next session's procedure, prepared |
| [rea.md](rea.md) | Interactive questions to the game's code through REA (Ghidra/Hopper as agent tools): its setup, its ELF targets, its traps |
| [data-index.md](data-index.md) | Every machine-readable artifact: path, schema, provenance, regeneration command |
| [open-questions.md](open-questions.md) | Named gaps. A known gap is cheaper than a rediscovered one |

## The method, in one paragraph

Fighter behaviour is not in the params, the animations, or the ACMD scripts — it is compiled C++
in `main` and compiled L2C in each fighter's `lua2cpp_<fighter>.nro`. So a ported move is written
by **reading the source fighter's decompiled status functions and transcribing them**, not by
describing what the move does and implementing that description (ADR 0006). Reconstruction from
params, wiki pages, and reasoning was tried first: Revenge was implemented wrongly five times, the
Alolan Whip's catch four, and the cargo carry four, because each round learned exactly one bit —
"still wrong" — against a hypothesis space that was not small (ADR 0005). Transcription is not
more careful; it is a different activity.

## Precedence, when two sources disagree

1. **`pikachu-moveset/docs/the-recording.md`** — 533 traced engine calls from the source fighter
   played on hardware. It wins over reasoning anywhere else in this repo, including over this
   handbook.
2. **The game's own code** — `main`'s `.text`, the fighter NROs, the decompiled C.
3. **The game's own data** — params, `motion_list.bin`, skeletons, the extracted ACMD.
4. **Another mod** (HewDraw Remix, George, the workshop episodes) — good for *how*, never for
   *what*. HDR is a rebalance mod, so its values are not the game's values.
5. **A wiki or a frame-data site** — corroboration only. When the code and a wiki disagree, the
   code wins.

A comment claiming "measured" is not evidence. Check that a log contains the measurement: one such
claim asserted `is_catch` never fires, and every line of the log it cited stopped one frame before
the window opened.

## What is new in this compilation

Three things that were open when this handbook was started and are closed in it:

- **The lua constant table is fully resolved for 13.0.5.** All 19,310 entries of
  `const_value_table__` now have a name *and* a value, so `ldr wN, [xTABLE, #0x250]` reads as
  `STATUS_KIND_NONE = 0xFFFFFFFF` without a running game. See
  [data-index.md](data-index.md). This closes the third argument of
  `CaptureModule::capture`, which two rounds guessed at and one explicitly declined to guess at.
- **The engine's function-boundary list is 100,508 entries, not 32,857.** The 67,651 extra are
  reached only through a vtable or a function pointer, which is precisely what a `bl` scan cannot
  see, and they were recovered from the relocated pointers in the live-process `.data`.
- **39,213 lua constants with their values**, recovered by emulating `main`'s own registration
  code with Unicorn rather than by inverting a hash. Eleven independent cross-checks against
  numbers this project had derived the hard way all pass; see [parity.md](parity.md#verification-of-this-handbooks-own-new-data).
- **All 100,508 of them are decompiled** — 380 MB, 900 seconds, 16 failures, and *zero* addresses
  Ghidra could not create a function at, which is the strongest available evidence that the
  pointer-derived set is real. The first thing it bought was `vtable[0x80]`, the real
  `CaptureModule::capture` implementation, which no `bl` reaches and which had been open since the
  first grab bug.

## Where the rest of the repo's knowledge lives

This handbook indexes the general knowledge. The project-specific record stays where it is, and
these are still the authority for their own subjects:

| Path | Authority for |
|---|---|
| `BACKLOG.md` | what is left to do, and the findings log, newest section last |
| `CONTEXT.md` | the domain vocabulary — host fighter, source fighter, slot, parity, reach, appendage, disjoint, agent, article, motion, move property, character-wide trait, parity table |
| `RESTORE.md` | the console revert procedure, per release |
| `pikachu-moveset/SPEC.md` | the moveset itself: 25 slots with their source fighters, and the eight borrowed attributes |
| `pikachu-moveset/docs/adr/` | the six decisions, with the reasoning that would otherwise be re-litigated |
| `pikachu-moveset/docs/decisions.md` | every design-interview round, including the gravity choice |
| `pikachu-moveset/docs/the-recording.md` | **533 traced engine calls from the source fighter on hardware.** Outranks everything else here |
| `pikachu-moveset/docs/reading-the-games-code.md` | the long-form version of [sources-of-truth.md](sources-of-truth.md), with the worked examples |
| `pikachu-moveset/docs/what-the-grabs-need.md` | ~40 rows classifying every grab behaviour as native, transcribed, substituted, or intercepted |
| `pikachu-moveset/docs/the-carry.md`, `the-regrab-report.md` | the two hardest moves, investigated in full |
| `pikachu-moveset/docs/the-native-audit.md`, `the-adversarial-pass.md` | the per-move audit against native, and the edge-case sweep |
| `pikachu-moveset/docs/the-data-analysis.md` | what the bulk data pull says, and three things it says are not wrong |
| `pikachu-moveset/docs/reference-recipe.md`, `api-reference.md`, `sword-options.md` | the George teardown, the `lua_bind` surface, and the weapon-asset options |
| `pikachu-moveset/docs/console-access.md`, `environment.md` | the console and the toolchain |

## What this handbook is not

It is not the backlog. Outstanding work stays in `BACKLOG.md`, which is authoritative for what is
left to do; this directory is authoritative for what is known. It is also not a substitute for the
ADRs — `pikachu-moveset/docs/adr/` records *decisions*, with the reasoning that would otherwise be
re-litigated, and those are still the place a decision gets changed.
