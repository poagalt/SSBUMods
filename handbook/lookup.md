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
| What does a status main loop / ACMD body (`&LAB_…` in its caller) do? | `code-index.py fn packun 0x71000336a0`: every pointer-only target is a function now ([below](#code-reached-only-by-pointer-added-2026-10-18)) |
| The exact instructions of a function | `code-index.py asm packun 0x71000336a0`, or `zgrep` over `work/code-index/export/<m>/asm.txt.gz` |
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

## Code reached only by pointer (added 2026-10-18)

Ghidra's auto-analysis starts functions where flow goes, never at an address code only takes the
address *of*. Every lua2cpp status main loop (`sub_shift_status_main(this, &LAB_…)`) and every
ACMD body (`sv_set_function_hash(this, &LAB_…, hash)`) is built that way, so until this pass none
of them had a decompile, and in the NROs that was 30% of the code.
`tools/ghidra/PointerFunctions.java` makes a function at every such target: a DATA, PARAM,
DATA_IND or INDIRECTION reference (or a call) into executable memory that no function contains.
It creates them **highest address first**, so an assert wrapper ending in a call Ghidra does not
know is noreturn cannot run on into the next entry and swallow it (ascending order lost 51 that
way on packun). Then it re-runs the program's own analyzers over the new code, round after round,
until a round finds nothing. `main` is linked at 0x0, so every small immediate there is also a
.text "address". For main the pass is therefore **strict**: a target must be a pointer the code
really builds, with an `adr`, or an `adrp`+`add` in the 1 KB before a referencing instruction, and
not sit mid-code. Failing that, an exact 8-byte pointer word in data must hold it, right after
padding, `ret`, `br`, `brk`, `udf` or `nop`. The rest are listed as rejected. A boundary-word test
alone was tried first and is not enough: small immediates carved the mid-function curl code at
0x3a00 into 4-byte "functions".

Measured 2026-10-18: the 96 NROs went from 144,987 to 266,504 functions (+121,517). The code
inside a function went from 80.6 to 114.6 MB, and every one of the 110,996 `&LAB_`/`&DAT_` code
pointers in their decompiles now lands on a function entry. `main` went from 104,558 to 112,207
(+7,649; 49.0 to 52.1 MB). Strict mode refused 1,067 mid-code targets there, 290 that no `adr`
or `adrp`+`add` builds, and 5 data words after a `b`/`bl`. Nothing in the old index was lost.
One main function (0x2a2d670) shrank when its tail-call target became a function, so its C was
redone. Per-module counts are in `work/code-index/pointer-functions-counts.tsv`.

Each function it made carries an ANALYSIS bookmark `PointerFunctions` in the kept project, and
`export/<m>/pointer-functions.tsv` lists them with the reference that found each one and the
instruction before it. A caller's decompile was not redone, so it still reads `&LAB_71000336a0`;
look up the address instead: `code-index.py fn packun 0x71000336a0`.

**Raw instructions** are in the index too: `code-index.py asm packun 0x71000336a0`, or
`zgrep -n '<pattern>' work/code-index/export/<m>/asm.txt.gz`. Each line reads
`addr: word  insn  ; -> what it references`. The file is gzipped, one member per 64 functions
(`asm.idx` gives the offsets), because plain it would be ~2 GB. `tools/disasm` is still the tool
for lua_const and PLT annotation.

Re-running: `bash tools/code-index.sh pointers nro|main|lua2cpp_<m>.elf` (it saves into the kept
project and writes an incremental export to `export.next/`), then `python tools/code-index.py
merge`, `swap`, and `build`. A fresh `code-index.sh export` runs the pass itself.
