# ADR 0009 — Ship facts about the game; build the game's data locally

Status: accepted, 2026-09-14.

## The decision

The builder ships **no game-derived content**. It ships code, fetchers for the community corpora,
an extractor for what only the user's own console can provide, and **derived facts** — statements
*about* the game rather than copies of it, such as the feasibility matrix and the constant table.

The **Game Database is built on the user's machine and cached**, never distributed. A separate
**private repo holds the development corpus**, scoped to what is expensive to *acquire* rather than
expensive to *compute*.

## Why

The corpus measures **~3.5 GB**: 364 MB of decompiled engine, 97 MB of dumped executable, 2.3 GB of
extracted archive, 493 MB of community repos, 263 MB of module decompiles. Shipping it means
redistributing Nintendo's code and assets — and this repository's own `.gitignore` already refuses
to version 380 MB of decompiled engine, on a **private** repo, reasoning that "it is also
Nintendo's code, which the eight tracked asset files already make awkward enough."

A public tool cannot hold a looser standard than the private project does. That settles it before
any legal question is reached.

## The rule for the private development corpus

**Store what is expensive to acquire, not what is expensive to compute.**

| | | in the repo? |
|---|---|---|
| `vendor/main/*` — `.text`, `.rodata`, `.data`, symbols | 97 MB | **yes** — needs a console dump, a plugin flip, and a match, every time |
| `work/emu/*` — 39,213 constants, the resolved const table, 100,508 entry points | 11 MB | **yes** — tiny, and derived from the above |
| the 8 Nintendo asset files currently tracked in `SSBUMods` | small | **yes** — and moving them is what would let `SSBUMods` go public |
| `work/ghidra/main-all-100k.c` | 364 MB | **no** — rebuilds in 15 minutes, and GitHub hard-refuses any file over 100 MB |
| community corpora (ACMD dumps, status decomp, param labels) | 493 MB | **no** — a copy of a public repo inside a private one is pure duplication |
| `extracted/**` | 2.3 GB | **no** — too large, and per-host anyway |

That narrow set is ~110 MB with every file under GitHub's limit.

## The distinction that makes this work

The private repo **cannot be the distribution path** — [ADR 0001](0001-a-tool-anyone-can-use.md)
says anyone can use the tool, and a private repo cannot serve anonymous users. So there are two
implementations of one interface, and the generator codes against the interface and never learns
which is behind it:

- **dev corpus** — one private repo, cloned, complete, including the decompiled C that the
  transpiler in [ADR 0004](0004-transpile-do-not-transcribe.md) has to read.
- **user corpus** — fetched from the community repos plus extracted from the user's own console on
  first run.

## Considered and rejected

**Ship everything.** Best user experience by a wide margin, and it redistributes Nintendo's code
and assets wholesale.

**Ship nothing at all, not even derived facts.** Rejected because the facts are the part that makes
the tool approachable: a matrix saying "Incineroar's side-B needs an article and a victim status"
is a statement *about* the game, not a copy of it, and it is what lets a user see what they are in
for before committing a console session. It is also what removes Unicorn from every user's machine
— the constant extraction happens once, upstream.

## Consequences

- **The Game Database carries a version fingerprint and refuses to mix dumps.** This project has
  already been bitten by exactly that: a `.data` dump from one ASLR session read against a `.text`
  base from another.
- **Ghidra remains a dependency for class-3 moves only**, and its absence denies that class
  gracefully rather than failing. Classes 1 and 2 need neither Ghidra nor Python.
- A new machine becomes one `git clone` plus one console session, rather than a day of
  re-acquisition.
- `SSBUMods` becomes publishable once the 8 tracked asset files move to the corpus repo. That is a
  side benefit, not a goal, but it is the only thing currently preventing it.
