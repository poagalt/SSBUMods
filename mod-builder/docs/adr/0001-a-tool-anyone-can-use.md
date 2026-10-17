# ADR 0001 — The builder is a tool anyone can use, not an instrument for one operator

Status: accepted, 2026-09-14.

## The decision

The builder targets **other people** — modders who want to put a fighter's move onto another
fighter — not an audience of one working beside a model with the whole corpus at hand.

This is the first decision and it constrains every other one, so it is recorded first.

## Why it matters more than it looks

The obvious plan for a tool of one is a **transcription scaffold**: for a move with its own status
machine, generate one module per status line the source actually has, pre-populated with the
source's decompiled C in comments and its ordered engine calls, and let the operator fill in the
bodies against a checklist. That is exactly what this project has been doing by hand for weeks,
and it works.

It is not a product. "Here is 700 lines of Ghidra output, go transcribe it" cannot be shipped to
someone who does not already have the decompile, the constant table, a console, and the habit of
reading engine C. So the moment the audience widened, the scaffold stopped being an acceptable
answer for the hard class of moves, and the hard class had to be either **solved** or **refused
with a clear reason**. That fork produced [ADR 0004](0004-transpile-do-not-transcribe.md).

## What else follows from it

- **The corpus cannot ship.** The builder reads ~3.5 GB of game-derived data — 364 MB of
  decompiled engine, 97 MB of dumped executable, 2.3 GB of extracted archive. Shipping it means
  redistributing Nintendo's code and assets, which this repository's own `.gitignore` already
  refuses to do on a private repo. So the tool ships code, fetchers for the community corpora, and
  an extractor, and builds its database on the user's machine. Facts *about* the game — the
  feasibility matrix, the constant table — may ship; copies of the game may not.
- **Dependencies became a design problem.** Building the database needs Ghidra plus a JDK (1.2 GB)
  and Python plus Unicorn. A modder does not expect to install a decompiler to make a mod, so the
  derived facts ship to eliminate Unicorn entirely, and Ghidra's absence gracefully **denies
  class 3** rather than failing.
- **Refusal became a first-class output.** Three different reasons now route through one path: the
  roster tiers of [ADR 0003](0003-declared-support-tiers.md), a source helper that has not been
  decompiled, and a missing local capability.
- **Verification cannot rely on playtesting.** The operator cannot playtest for their users, so the
  bar is offline completeness plus an in-game self-test whose verdict the user reads locally.

## The alternative, and what it would have bought

An audience of one is much cheaper. The scaffold is already proven, articles could stay
out of scope, the corpus could just sit on disk, and Ghidra being required would be a non-issue.
Most of the cost in this plan exists because of this decision.

It was taken anyway because the tool's value is in *other people* making characters, and a tool
that only its author can run is the project that already exists.

## Consequences worth flagging

The builder's output question is now open in a way it would not have been: `pikachu-moveset`
replaces a roster member agent-wide, which is almost certainly the wrong default for someone
else's mod. Whether a generated mod replaces a fighter or occupies a costume slot is under
investigation and will get its own ADR.
