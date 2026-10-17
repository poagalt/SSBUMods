# The data index

Every machine-readable artifact this project has, with its path, schema, row count, provenance,
and the command that regenerates it.

**Why the data is not in this directory.** `.gitignore` states the policy and gives the reason:
Nintendo-derived output that a script reproduces is not versioned, because it is large and because
"it is also Nintendo's code, which the eight tracked asset files already make awkward enough."
That policy is right and this index respects it — the recipe is versioned, the bytes are not. So
this page is the map to `work/` and `vendor/`, both of which are gitignored, both of which rebuild
from the commands below.

Anything below marked **new (2026-09-14)** did not exist before this compilation.

---

## The engine's own constants

### `work/emu/consts-1305.csv` — **new (2026-09-14)**

**39,213 lua constants for 13.0.5, with names and values.**

```
# kind,hash40,name,value,registering_function
CONST,0x1A55688462,"FIGHTER_STATUS_KIND_ATTACK",0x27,0x20fbb90
CONST,0x2B09744E41,"FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT",0x1100000A,0x2123d40
```

`hash40` is computed, not read — `(len << 32) | crc32(name)`, the same formula `tools/hash40.sh`
uses. `registering_function` is the module-relative address of the function that registered it,
which is useful when a constant's meaning depends on its neighbours.

```bash
python tools/emu-module.py consts        # ~2 s, 55 functions, all clean
```

Provenance: emulating each of 13.0.5's 55 constant-registering functions with Unicorn and reading
the arguments of its 40,338 calls to the register function at `0x37289c0`. The 1,125-entry
difference between 40,338 calls and 39,213 constants is duplicate names. Verified against eleven
values this project had derived independently; see [parity.md](parity.md#verification-of-this-handbooks-own-new-data).

### `work/emu/lua-const-table-1305.tsv` — **new (2026-09-14)**

**The const table, fully resolved: 19,310 of 19,310 offsets have a name and a value.**

```
# offset	name	value
0x250	STATUS_KIND_NONE	0xFFFFFFFF
0x4e4	LINK_NO_CAPTURE	0x0
0x9cc	FIGHTER_STATUS_KIND_CAPTURE_CUT	0x44
```

This is the join that makes a disassembly readable. Every status function reads its constants as
`ldr wN, [xTABLE, #off]` out of one i32 array, and until now an offset could be given a *name*
(from `lua_dict.txt`) but not a *value* without a running game. Both columns are now static.

It also closes the third argument of `CaptureModule::capture`, which was `lua_const[0x250]` for
the whole project — two rounds guessed at it and one explicitly declined to:
**`STATUS_KIND_NONE = 0xFFFFFFFF`**.

```bash
python tools/emu-module.py table         # needs `consts` to have run first
```

Offset → name comes from `vendor/ssbu-status/lua_dict.txt`, name → value from
`consts-1305.csv`. The offsets in `lua_dict.txt` are **hex without a `0x` prefix** — parsing them
as decimal fails on the third line of the file.

### `vendor/ssbu-status/lua_dict.txt`

19,310 `offset:NAME` lines, hex offsets, no prefix. Recovered by `disasm --got` naming
`const_value_table__` and working back from `sv_set_status_func` call sites. Cross-checked three
independent ways (see [parity.md](parity.md#verification-of-this-handbooks-own-new-data)).

### `vendor/main/lua-const-names.txt`

38,729 constant names, no values — superseded for value lookups by `consts-1305.csv`, which
contains 38,726 of them plus 487 more that are 13.0.5-only. Three names are in this list and not
in the extraction.

---

## Function boundaries

### `work/emu/all-entrypoints.txt` — **new (2026-09-14)**

**100,508 module-relative function entry points in `main`**, one `0x%010x` per line, sorted. The
union of two sources:

| Source | Count | Reachable how |
|---|---|---|
| `work/acquire/main-entrypoints.txt` | 32,857 | 25,928 distinct `bl` targets from 537,516 `bl` instructions, plus 6,929 symbols nothing calls directly |
| `work/emu/ptr-entrypoints.txt` | 70,143 | relocated pointers into `.text` found in the live-process `.data` and `.bss` |
| `work/emu/new-entrypoints.txt` | 67,651 | the pointer set minus the `bl` set — **vtable slots and function-pointer tables**, which is exactly what a static `bl` scan cannot see |

For scale, `module-info-extractor` published 99,220 for 2.0.0 by a comparable method.

**`work/acquire/main-entrypoints.txt` is a live input, not a leftover.** `emu-module.py`'s
`known_entrypoints()` reads it to define what "new" means, so the 67,651 in the third row exist
only relative to it. `the-emulation-audit.md` item 5 called it "now stale and should say so, or
go"; deleting it would break the `ptrs` subcommand. It is a baseline, and baselines are kept.

**Decompiled as of 2026-09-15: 100,539 of these have bodies on disk** —
`work/ghidra/main-all-100k.c` (100,508 delimited bodies, 16 decompilation failures, 900 s) plus
`work/ghidra/main-ind-31.c` (the 31 below, 3 s). Both are rebuildable and neither is tracked.

**Why the 67,651 are believed to be functions.** 289,768 of 289,770 pointers are 4-aligned, and a
sample of 3,000 of the new entries against 3,000 known `bl` targets classifies by opening
instruction like this:

| Opening | new (from `.data`) | known (`bl` targets) |
|---|---|---|
| `sub sp, sp, #imm` | 14.5% | 37.6% |
| `stp …, [sp, …]` | 7.8% | 13.5% |
| `ret` (empty body) | **10.8%** | 1.0% |
| `adrp` (leaf) | 4.1% | 6.4% |
| `b` (thunk or tail call) | 1.3% | 0.5% |
| unclassified | 61.5% | 40.9% |

The new set has fewer stack-frame prologues, which is expected — direct `bl` targets skew toward
large non-leaf functions, while vtable slots skew small — and a **tenfold excess of bodies that
are a bare `ret`**, which is what an empty virtual override looks like in C++. That is the
signature the hypothesis predicts. The caveat is that the classifier is crude: 40.9% of
*known-good* entries are unclassified too, so this supports the population and cannot be used to
filter individual addresses.

```bash
tools/disasm/target/release/disasm.exe --flat vendor/main/rat-main-text.bin --callees   # the 32,857
python tools/emu-module.py ptrs                                                          # the 70,143
```

`decomp-many.sh` takes an address list from a file precisely so a list this size can be handed to
it; 32,857 functions decompiled in 500 s across 24 cores.

### `work/emu/indirect-targets.txt`

Indirect call and jump destinations resolved by emulating all 100,508 entry points under a
20,000-instruction budget (about 17 minutes at ~100 seeds/s). Lines are
`<target> <blr|br> from <seed>`.

**Read the tag** — it is the whole point of the file. A `blr` target is a call destination and
therefore a function entry by definition; a `br` target is often a switch-table label or a tail
call *inside* an existing function. The tagged run, 2,063 destinations:

| Tag | Targets | Already known | New | New with a stack prologue |
|---|---|---|---|---|
| `blr` (a call) | 1,025 | **994 (97%)** | 31 | 7 |
| `br` (a jump) | 1,038 | 371 (36%) | 667 | 6 |

**So the sweep's value is validation, not discovery.** 97% of the addresses it resolved as call
targets were already in the list, which is a strong independent check on both the emulation and
the pointer scan. Its net contribution to the boundary list is **31 addresses**
(`work/emu/indirect-new-calls.txt`) — 0.03% of the total. The 667 new `br` targets are almost
entirely jump-table labels and must not be treated as boundaries; an untagged first run reported
724 "new entry points" of which only 12 had a prologue, which is what the tag exists to prevent.

```bash
python tools/emu-module.py indirect      # resumable; skips seeds listed in indirect.progress
awk '$2=="blr"{print $1}' work/emu/indirect-targets.txt   # the only lines worth reading as boundaries
```

The 31 were decompiled on 2026-09-15 to `work/ghidra/main-ind-31.c` (3 s, 0 failures).

**AND THE WARNING ABOVE HAS TEETH, because the Ghidra project persists.** Merging all 698 and
decompiling them was tried first, on the strength of `the-emulation-audit.md` item 2 rather than
this table. `DecompMany` calls `createFunction` at every address it is given, and a function
created at a mid-function `br` label makes Ghidra split or refuse the real enclosing function —
so that run left **667 spurious boundaries inside `work/ghidra/rat.rep`**, which
`decomp-many.sh` then reuses via `-process` on every subsequent run. The damage is not in the
output file, which can simply be discarded; it is in the shared program database.

Recovery is cheap only because the database is derived: `rm -rf work/ghidra/rat.rep
work/ghidra/rat.gpr work/ghidra/.imported` and the next invocation re-imports
`vendor/main/rat-main-text.bin` in seconds under `-noanalysis`. Do that before trusting any bulk
run that a bad address list has touched.

### `vendor/main/symbols-merged.txt`

8,906 `offset size name` lines: the 4,917 exports dumped out of the process merged with the public
decomp's 15,320 names for 13.0.1. The export list alone is **not** a function list — it is
2,225 `app::lua_bind::*`, 248 `app::ai`, 219 `sv_animcmd`, and zero hits for `FighterGaogaen`.

---

## The game's own code

| Artifact | Size | Command |
|---|---|---|
| `vendor/main/rat-main-text.bin` | 60,592,128 B | `tools/pull-exedump.sh` (flip `ENABLED`/`DUMP_TEXT`, load one match) |
| `vendor/main/rat-main-{rodata,data,bss}.bin` | 22.3 / 3.7 / 6.8 MB | same. `.data`/`.bss` are **relocated for base `0x4dd3c04000`** |
| `work/ghidra/main-all-100k.c` | **380 MB, 100,508 functions** | `decomp-many.sh main work/emu/all-entrypoints.txt work/ghidra/main-all-100k.c` — 900 s, 16 decompilation failures (0.016%), **0 addresses with no function to create** |
| `work/ghidra/main-all.c` | 149 MB, 32,857 functions | the earlier `bl`-targets-only run, kept for diffing |
| `vendor/nro/prebuilt/nro/release/*.nro` | **20 agents**, including `lua2cpp_common` | `tools/pull-nro.sh`, game running. 98 exist in the arc; pull one when the decomp omits what you need |
| `vendor/ssbu-status-all/` | 124 MB, 95 fighters | `tools/fetch-status-scripts.sh` |
| `vendor/decomp/` | 18 module trees | `tools/decomp-all.sh` / `decomp-sweep.sh` |
| `vendor/ssbu-decomp/` | 15,320 named + 39,636 sized | clone `sbergeron42/ssbu-decomp` |
| `work/code-index/ssbu-code.sqlite` | main + 96 NROs: functions, symbols, code and data references, strings, decompile | `bash tools/code-index.sh all` ([lookup.md](lookup.md)) |
| `work/move-index/ssbu-moves.sqlite`, `sheets/<f>.md` | per-fighter motions, ACMD events, hitboxes, effects, statuses, params, assets | `python tools/move-index.py build` |
| `work/lookup/ssbu-lookup.sqlite` | constants, hash names, community names, docs full-text | `python tools/ssbu.py build` |
| `work/rea/main.elf`, `work/rea/lua2cpp_<m>.elf` | 87 MB / 103,596 symbols for `main` | `python tools/rea-elf.py all` — about 2 s for `main`. Each module wrapped as an AArch64 ELF from its own MOD0, `.dynsym` and `.rela`, for REA ([rea.md](rea.md)); a `.json` manifest beside each |

---

## The game's own data

| Artifact | Count | Command |
|---|---|---|
| `work/acquire/arc-paths.txt` | 590,711 paths, 590,707 named | `tools/arc-pull --list`, game running |
| `extracted/fighter/**` | params 9,243/9,243 · motion lists 5,660/5,660 · skeletons 7,208/7,208 · one full `c00` (40,264 files, 1.27 GiB) | `tools/arc-fetch.sh <path-list>` |
| `vendor/refs/param-labels/ParamLabels.csv` | hash → name for every param field | `tools/fetch-refs.sh` |
| `vendor/refs/SSBU-Dumped-Motion-Lists` | every vanilla `motion_list` | same |
| `vendor/SSBU-Dumped-Scripts` | every vanilla ACMD, 309 MB | clone `WuBoytH/SSBU-Dumped-Scripts` |
| `vendor/{Hashes,ArcHashes}.txt` | 149,871 community names | — |
| `vendor/main/hash40-dict.txt` | 47,858 hashes of identifier-like strings in `main` | `luaconst --hashdict` |
| `vendor/main/hash40-confirmed.txt` | each guessed name with the constant it was confirmed against | by hand, on confirmation |

`Hashes.txt` holds path **components**, not full paths — 151 of 590,711 resolve directly, while
`FilePath.parent` plus `file_name` reassembles 583,549 of them (98.8%).

---

## The port's own manifests

Three hand-maintained lists that must agree, which `tools/check-lists.sh` enforces because they
have disagreed:

| File | Rows | Holds |
|---|---|---|
| `tools/moves.tsv` | 60 lines (52 move rows) | `source · acmd file · acmd fn · animation stem · plugin module · label`. Tab-**named**, space-**aligned** — read with bare `read -r`, never `awk -F'\t'` |
| `tools/scales.tsv` | 96 | every fighter's `scale` param, generated from the game's `fighter_param.prc`, never hand-written |
| `tools/ported-motions.txt` | 114 | the shipped motion artefact, one list replacing three parallel ones |

`gen-moves.sh` also keeps its **own** inline group lists, and `topscale`'s `MAP` is a third.
All three are kept in step by hand, and by that check.

## The recordings

| File | Holds |
|---|---|
| `pikachu-moveset/docs/the-recording.md` | 533 engine calls traced off the **source fighter** played natively. Outranks reasoning anywhere in the repo |
| `work/pt*.log` | per-playtest telemetry, pulled by `analyze.sh`. Rotated to `.prev` rather than truncated, after a truncation destroyed a crashed session's evidence |
| `work/crash*.log` | Atmosphère crash reports |

---

## Regenerating everything from an empty tree

```bash
bash tools/fetch-refs.sh              # community reference data
bash tools/fetch-status-scripts.sh    # 95 fighters' status decomp
# --- console, game running, arc_network loaded ---
bash tools/pull-nro.sh                # fighter NROs
bash tools/arc-fetch.sh <paths>       # params, motion lists, skeletons, animations
bash tools/pull-exedump.sh            # main's text/rodata/data/bss + symbols
# --- offline from here ---
python tools/emu-module.py ptrs        # 100,508 entry points
python tools/emu-module.py consts      # 39,213 constants with values
bash tools/decomp-many.sh main work/emu/all-entrypoints.txt work/ghidra/main-all.c
python tools/rea-elf.py all            # REA targets, work/rea/*.elf
```

Everything else derives from those. If a step needs the game running, it says so; if it needs
`prod.keys`, it cannot be done at all on this machine and the running game is the route instead.
