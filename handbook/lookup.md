# Looking things up

One command answers "what is this?" about the game, from every source the project holds:

```bash
python tools/ssbu.py find <anything>      # a name, an address, a hash40, a work id, a param
```

It prints one line per hit, each tagged with where the fact came from, so a reader can apply
[README.md](README.md)'s precedence (recording > the game's code > its data > another mod > a wiki).
Built 2026-10-12, offline, from what is already on this PC.

## The three indexes underneath

| Index | Built by | Holds |
|---|---|---|
| **code** `work/code-index/ssbu-code.sqlite` | `bash tools/code-index.sh all` (once; ~1 h for `main`) | `main` and 96 fighter NROs after a full Ghidra analysis of `work/rea/*.elf`: functions with signatures, every symbol (the module's own names, demangled, plus status functions as `<KIND>__<FUNC>`), **every code and data reference**, every string, and a decompile per module |
| **moves** `work/move-index/ssbu-moves.sqlite` | `python tools/move-index.py build` (minutes) | per fighter and agent: c00 motions (animation, frame count, cancel frame, intangibility), every ACMD script parsed into timed events, hitboxes with their active windows, effects with colours, sounds, article spawns, the status functions each NRO registers and which statuses play which motion, `vl.prc` and `fighter_param.prc` values, and every asset the arc lists with whether it is on this PC |
| **lookup** `work/lookup/ssbu-lookup.sqlite` | `python tools/ssbu.py build` (seconds) | all 19,310 lua constants with values, 39,213 emulator-recovered constants, ~289k hash40 names, 11,389 community function names for `main`, and full-text search over 337 docs and references |

`tools/ssbu.py` attaches the other two, so `find` searches all three at once.

## Questions, and the command that answers each

| Question | Command |
|---|---|
| What is `0x200000ea`? | `ssbu.py find 0x200000ea`: every constant with that value, which is how the instance-work-id collisions in [traps.md](traps.md) show up |
| What does this function do, and who reaches it? | `code-index.py fn main 0xabe800`, `callers`, and **`refs-to`**, which includes pointers in data: vtables, spawn tables, status tables |
| Which code uses this string? | `code-index.py strings main "<text>"` |
| Everything about one move | `move-index.py move <fighter> <motion>`, or the fighter's sheet in `work/move-index/sheets/` |
| What is this hash40 / what is this name's hash? | `ssbu.py hash <name or 0x…>` |
| A lua constant, by name or by value | `ssbu.py const <name or value>` |
| Where did the project write about X? | `ssbu.py doc "<words>"` |
| What does this PC lack for fighter X? | `move-index.py missing <f> --list model` |
| What sources exist at all? | `ssbu.py sources` |

## Outside sources, and how far to trust them

| Source | In the index as | Trust |
|---|---|---|
| `vendor/ssbu-decomp` (sbergeron42's matching decomp) | `community` names for `main` | Labelled 13.0.1, but **511 of the 518** exports it shares with this project's 13.0.5 dump sit at the same address (measured 2026-10-12). Names, not facts |
| `vendor/SSBU-Dumped-Scripts` | the move index's ACMD | Dumped from the game's NROs; it is what `tools/gen-moves.sh` copies verbatim |
| `vendor/ssbu-status-all` | status → motion links | Ghidra decompiles of the NROs; a status "plays" a motion only when its own C holds that motion's hash |
| `vendor/hdr`, `ssbuGeorgeMoveset`, `ssbu-mod-workshop` | docs search | another mod: good for *how*, never for *what* |
| `vendor/smashwiki` | docs search | corroboration only |

## Rebuilding

The order matters only because each layer reads the previous one:

```bash
python tools/rea-elf.py all          # ELFs from vendor/ + work/arc-nro (+ extracted/prebuilt after a dump)
bash tools/code-index.sh all          # background; ~1 h
python tools/move-index.py build
python tools/ssbu.py build
```

After the next Switch session ([switch-session.md](switch-session.md)), `post-dump` runs the
first and third steps itself.
