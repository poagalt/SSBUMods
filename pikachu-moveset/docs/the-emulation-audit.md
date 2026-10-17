# What the Unicorn emulation actually produced, and what to do with it

Read on 2026-09-14 at the user's request, against the artefacts in `work/emu/` and the proposal in
`mod-builder/PLAN.md`. Both are another agent's work; nothing here edits either.

## It works, and there is a hard test for that

The claim worth testing was the one the whole exercise exists for: `--callees` collects `bl`
targets, so a function reached only through a vtable has no entry point and the decompiler cannot
be handed it. `docs/the-native-audit.md` names the cost -- the corpus has a hole from 0xab86f0 to
0xab9800, and the Revenge HIT/TURN transition lives inside it, which is why finding that branch
took a byte scan of `.text` for a MOVZ encoding rather than a `grep`.

    window 0xab8700..0xaba000
      work/acquire/main-entrypoints.txt (old, bl-only)   1 entry point   0xab9800
      work/emu/all-entrypoints.txt      (new)            7 entry points  ..88e0 ..8a20 ..8f30
                                                                         ..9140 ..9800 ..9970 ..9c50

The branch at 0xab9f14 now sits inside a known function. **The blind spot this was built to close
is closed**, and had the corpus been rebuilt first, today's Revenge work would have read C instead
of instructions.

## And the constant table is independently correct

`work/emu/lua-const-table-1305.tsv` is 19,310 rows of `offset -> name -> VALUE` for 13.0.5's own
`const_value_table__`. Every one of the twenty-one constants this session resolved by hand -- out
of two different fighters' helpers, by three different methods -- agrees with it:

    0x1274 DAMAGE_LEVEL_FLY 0x3        0xa8c/0xa9c/0xaa0 DAMAGE_LEVEL_1/2/3 0x0/0x1/0x2
    0x410  FIGHTER_KINETIC_TYPE_GROUND_STOP 0x6     0x430 FIGHTER_KINETIC_TYPE_FALL 0x0
    0x3b4  ..._ENERGY_ID_GRAVITY 0x1    0x498 _STOP 0x3   0x4a8 _CONTROL 0x2   0x274 _MOTION 0x0
    0x49c/0x4a4 ENERGY_STOP_RESET_TYPE_GROUND/_AIR 0x0/0x7
    0x280c/0x2810 ENERGY_MOTION_RESET_TYPE_GROUND_TRANS/_AIR_TRANS 0x2/0x4
    0xb574/0xb578 FIGHTER_GAOGAEN_STATUS_KIND_SPECIAL_LW_HIT/_TURN 0x1EB/0x1EC
    0xb608 ..._FLAG_INVALID_SPECIAL_AIR_LW_SPPED_Y 0x200000E6    0xd0 ..._SPECIAL_LW 0x1DF

That is a two-way check: it corroborates the transcriptions committed today, and it corroborates
the emulator against a set of values nobody derived from it.

## THE ONE THING THAT IS NOT DONE

> **SUPERSEDED on 2026-09-15 — this section was wrong when it was written. See the addendum at
> the end of this file.** The decompilation HAD already run: `work/ghidra/main-all-100k.c`, 100,508
> delimited bodies, finished 2026-09-14 03:31, about 28 minutes after `indirect-targets.txt` was
> written. This section read `vendor/decomp/main` and never looked in `work/ghidra`. The reasoning
> below about *which* number a load-bearing row states is still worth keeping; the premise is not.

**The decompilation has not run over any of it.** `vendor/decomp/main` still holds 32,857 files,
generated from the old `bl`-only list. So the capability is complete and the corpus is not, and
`mod-builder/PLAN.md`'s table row

    | Engine functions decompiled | 100,508 |

is entry points IDENTIFIED, not functions decompiled. The real figures are 100,508 identified
against 32,857 decompiled. That row is load-bearing in a plan whose whole argument is about which
work is mechanisable, so it should say which number it is.

## Improvements, in order of value

**1. Decompile the 67,651 new addresses.** This is the deliverable and it is the only step left.
`tools/decomp-all.sh` is resumable, `tools/ghidra/DecompMany.java` now does a whole address list in
one JVM on all 24 cores, and `work/emu/new-entrypoints.txt` is exactly the input it wants. Every
"only reached through a vtable, so we cannot read it" note in `the-native-audit.md` is waiting on
this and nothing else.

**2. Merge `indirect-targets.txt` first -- it is free and it is disjoint.** 2,063 addresses found
by sweeping all 100,508 functions for indirect calls, and **all 2,063 are absent from
`all-entrypoints.txt`**. Nobody folded the second round back in. That is 102,571 entry points
before a single extra second of emulation.

> **CORRECTED 2026-09-15: the merge is 31 addresses, not 2,063, and they are not disjoint.**
> 1,365 of the 2,063 were already in `all-entrypoints.txt`. Of the 698 that were not, only the
> **31** tagged `blr` are call targets and therefore function entries; the other 667 are `br`
> jump-table labels and merging them would be actively wrong. The total is **100,539**, not
> 102,571. Two separate mistakes: the CRLF trap this file documents two sections down, and
> ignoring the `blr`/`br` tag. `handbook/data-index.md` had both right already and this section
> never consulted it -- read it before acting on this item.

Compare the FIRST COLUMN ONLY. `indirect-targets.txt` rows are `0x... blr from 0x...` and
`all-entrypoints.txt` rows are bare addresses, so a whole-line `comm` between them cannot match
and reports every row as new whatever the truth is. That is how this number was first obtained,
and it was right by luck:

    awk '{print $1}' work/emu/indirect-targets.txt | sort -u > /tmp/ind
    sort -u work/emu/all-entrypoints.txt > /tmp/all
    comm -12 /tmp/all /tmp/ind | wc -l    # 0 already present
    comm -13 /tmp/all /tmp/ind | wc -l    # 2063 new

> **It was not right by luck -- it was wrong, and by this exact mechanism.** `awk '{print $1}'`
> strips the CR from `ind`, because the CR is trailing whitespace of the last field. The bare
> `sort -u` on `all-entrypoints.txt` leaves it on. So every comparison is `0x...` against
> `0x...\r`, which can never match, which is why it printed `0 already present`. Strip both
> sides and it is 1,365 already present, 698 new:
>
>     tr -d '\r' < work/emu/indirect-targets.txt | awk 'NF{print $1}' | sort -u > /tmp/ind
>     tr -d '\r' < work/emu/all-entrypoints.txt  | awk 'NF{print $1}' | sort -u > /tmp/all
>     comm -12 /tmp/all /tmp/ind | wc -l    # 1365 already present
>     comm -13 /tmp/all /tmp/ind | wc -l    # 698 new
>
> Cross-checked the other way too: each address the corrected `comm` calls "already present"
> does return a hit from `grep -x` against the CR-stripped `all-entrypoints.txt`. The next
> section had this diagnosis exactly right, and then exempted its own number from it.
>
> **And 698 is still not the merge.** Splitting by the tag gives `blr` 1,025 total / 994 known /
> **31 new**, and `br` 1,038 / 371 / **667 new** -- which reproduces `handbook/data-index.md`'s
> table exactly. Only the 31 are boundaries. `work/emu/indirect-new-calls.txt` is that curated
> list and was sitting in the same directory the whole time.

**3. Point `disasm` at the new table.** `lua_const_note` currently reads
`vendor/ssbu-status/lua_dict.txt`, which is names only, and its own doc comment calls the result
"A HINT, NOT A FACT" -- any `ldr wN, [xM, #off]` gets a name it may have nothing to do with. The
new TSV carries the VALUE too, so the annotation can print `~lua_const NAME = 0x3` and a wrong
guess becomes visible instead of plausible. `RAT_LUA_DICT` already makes the path swappable; the
parser needs one extra column.

**4. Check skyline-smash's constants against the game's.** Every `*FIGHTER_KINETIC_TYPE_*` and
`*DAMAGE_LEVEL_*` in this plugin takes its VALUE from a third-party crate table. `consts-1305.csv`
has 39,213 `name,value` pairs recovered from the running game's own registration code, so a
`tools/check-luaconst.sh` diffing the two turns "the crate says 3" into a verified fact. This is
the class of error that has already cost this project four wrong entries in a hand-written
status-name table, and "never invent a number" currently stops one level short of the numbers
themselves.

**5. Move the durable artefacts out of `work/`.** `lua-const-table-1305.tsv` and `consts-1305.csv`
are findings, not run output, and `work/` is scratch and untracked. `vendor/main/` is where the
game's own data already lives. While there: `work/acquire/main-entrypoints.txt` is now stale and
should say so, or go.

## One measured shortfall, recorded rather than smoothed over

`work/emu/consts.log` line 55 of 55:

    [55/55] 0x225b990 +0x105e0  expected 2897 got 2092 (39213 total) 15946 insns ok

The last registering function yielded 2,092 of an expected 2,897 constants -- about 805 short, so
the 39,213 total is ~98% of what that pass should have found. Every other one of the 55 matched
exactly. Worth knowing before anything treats `consts-1305.csv` as complete: a NAME missing from
it is not evidence that the constant does not exist.

## Two things found while using it on a real question

Both came out of chasing the Alolan Whip's lariat, where the answer turned out to be an engine-side
victim transition at `main` 0x637ec4 -- the same shape as the Revenge HIT/TURN branch.

**The expansion does not cover that function, and neither does the indirect sweep.** The whole
64 KB region 0x630000..0x63ffff contains exactly ONE entry point in either list -- 0x6307f0, which
is a 137-line list lookup, not the dispatcher that holds 0x637ec4. The 2,063 indirect targets do
not reach it either. So the 3.1x expansion is real and was proven useful elsewhere (the Gaogaen
window went 1 -> 7), but it is not uniform, and a specific question can still land in a hole. The
honest framing is "three times the coverage", not "the blind spot is gone".

**Every file in `work/emu/` is CRLF, and that silently breaks `$` anchors.** This cost a wrong
answer during the above: `grep -cE "^0x000063[0-9a-f]{4}$"` returns 0 on all of them because the
line ends `...\r`, and 0 reads exactly like "no entry points in that range" rather than like a
parsing failure. Anything consuming these needs `tr -d '\r'` first, and any checker built on them
should assert the line count is non-zero rather than trusting a zero.

    tr -d '\r' < work/emu/all-entrypoints.txt | grep -cE "^0x[0-9a-f]{10}$"     # 100508

That belongs in whatever runs the bulk decompilation over `new-entrypoints.txt`, because a
silently-empty address list would look like a finished job.

---

# Addendum, 2026-09-15: every item above worked, and two of its numbers were wrong

Written while the console was unusable -- it had been updated from 22.5.0 to 23.0.0 and Atmosphère
supports 22.5.0 at most -- which is why this was a good day to do the five items and none of the
in-game work. Each is settled below, with what was actually measured.

## The headline claim was false

`work/ghidra/main-all-100k.c` already existed: **380,819,623 bytes, 100,508 `/* ==== 0x... */`
delimiters, 16 decompilation failures, 0 empty bodies**, and its own progress file records
`100508/100508 decompiled, 112/s, 900s elapsed`. It is dated 2026-09-14 03:31 -- twenty-eight
minutes after `indirect-targets.txt` (03:07), so it landed while this audit was being written.

The audit checked `vendor/decomp/main`, found 32,857 files, and concluded the corpus did not
exist. Those 32,857 are the **per-function tree** that `decomp-all.sh` writes from the symbolised
list; the bulk corpus is one file that `decomp-many.sh` writes, and nothing looks in `work/ghidra`.
Two artefacts, two shapes, and only one of them was checked. `mod-builder/PLAN.md` was right --
it names `work/ghidra/main-all-100k.c` and its 364 MB explicitly, four hundred lines further down
its own page than the row this audit called wrong.

**The lesson is narrower than "check twice".** A corpus that lives in scratch with no index is a
corpus nobody can find. `work/` is untracked, so the 363 MB of C that answers the question this
project keeps asking was invisible to a careful reader of the same repo on the same day.

## Item 2's number, corrected twice -- and the second correction was mine

The set difference is 1,365 already present and 698 absent, not 0 and 2,063. That part is the
CRLF trap, and the mechanism is in the blockquote above. The irony is exact: this file diagnoses
that trap correctly in "Two things found while using it on a real question", and the number in
item 2 was produced by it three sections earlier.

**Then I merged all 698 anyway, and that was wrong.** `handbook/data-index.md` already carried
the tagged breakdown, and recomputing it from the raw file reproduces it to the digit:

| Tag | Targets | Already known | New |
|---|---|---|---|
| `blr` (a call) | 1,025 | 994 | **31** |
| `br` (a jump) | 1,038 | 371 | **667** |

A `blr` target is a call destination and therefore a function entry. A `br` target is usually a
switch-table label or a tail call *inside* an existing function, and data-index says in bold that
those **must not be treated as boundaries** -- it records an untagged first run that reported 724
"new entry points" of which 12 had a prologue, which is exactly what the tag exists to prevent.

So the merge is **31**, the total is **100,539**, and `work/emu/indirect-new-calls.txt` was the
curated 31 all along, sitting in the same directory as the file I recomputed from.

**The lesson is about which document to trust.** This audit is one agent's reading of the
artefacts; `handbook/data-index.md` is the index of the artefacts themselves. When the two
disagree about what a file contains, the index wins, and item 2 above is what taking the audit's
word for it costs.

## Item 1: done, and one thing had to be undone

The bulk corpus was already complete (above). The legitimate increment was the 31:

    bash tools/decomp-many.sh main work/emu/indirect-new-calls.txt work/ghidra/main-ind-31.c
    # prepared 31, decompiled 31 in 3s, 0 failures, 169,106 bytes

**A first attempt decompiled all 698 and had to be rolled back.** Creating a function at a
mid-function `br` label makes Ghidra split or refuse the real enclosing function, and
`work/ghidra/rat.rep` PERSISTS between runs -- `decomp-many.sh` uses `-process` whenever the
program is already imported. So 667 spurious boundaries were left in the shared program database,
where they would have degraded every later decompilation of the functions that contain them.

The remedy, and the reason this is recoverable at all: `work/ghidra` is derived. Deleting
`rat.rep`, `rat.gpr` and `.imported` makes the next run re-import from
`vendor/main/rat-main-text.bin`, which with `-noanalysis` costs seconds. That was done, the 31
were decompiled on the clean project, and the bad output is parked as
`main-ind-698.c.SPURIOUS-667-DO-NOT-USE` rather than deleted, so the mistake is legible.

So the corpus is **100,539 entry points with decompiled bodies** across `main-all-100k.c` and
`main-ind-31.c`. Every "only reached through a vtable, so we cannot read it" note in
`the-native-audit.md` is now answerable by grep.

## Item 3: was already done

`lua_const_note` in `tools/disasm/src/main.rs` already formats `; ~lua_const NAME = 0x..` when the
dict carries a value, `lua_dict()` already returns `(String, Option<u64>)`, and it already prefers
`vendor/main/lua-const-table.tsv` over `vendor/ssbu-status/lua_dict.txt` with `RAT_LUA_DICT` as the
override. Both file formats are accepted. Verified live: 27 annotations in the window at
`0x637000`. `--flat` is required for `rat-main-text.bin`; without it the tool asserts on the
missing `NRO0` magic.

## Item 4: done, and it found two real defects

`tools/check-luaconst.sh` is new, and is wired into `tools/check.sh` as section **4m** -- next to
4c, because 4c checks *which* constants the code names and this checks *what they are worth*.

    crate: 38743 constant(s)   game: 39213 constant(s)
    of those, 267 are named by the plugin
    compared 38740 name(s) present in both
    ok  every constant the plugin names agrees with the game's own registration

**All 267 constants this plugin compiles in are correct.** That is the answer the item wanted and
it is now a gate rather than a belief.

Two rows of `skyline_smash`'s table do disagree with the game, and neither is referenced anywhere
in this repo:

    FIGHTER_EFLAME_STATUS_SPECIAL_HI_TERM     crate=0x2100000F  game=0x1000008
    FIGHTER_STATUS_TRAIL_REBOUND_WORK_TERM    crate=0x1000008   game=0x1000009

Three design notes that are load-bearing:

- **Absence is not failure.** `consts.log` records the last registering function 805 constants
  short, so the recovered set is ~98% complete and a missing NAME is evidence about the recovery,
  not about the constant. Only a disagreement gates.
- **Only the top-level `lua_consts!` invocation is compared.** `lua_const.rs` ends with ~40 more
  invocations nested in `pub mod AttackerAttribute { ... }` blocks whose names are short and
  module-scoped (`NONE: 0`, `ATTACK: 1`). Folding those in would collide `NONE` against whatever
  the game registers under that name. Top-level entries are 4-space indented and have zero
  duplicates; nested ones are 8-space. That indent is the discriminator.
- **The "game registers what the crate does not" count is split, because the flat number misreads
  by 6x.** It is 473, of which **406 are in those nested blocks** and only **67** are genuine
  gaps. A bare "473 the crate does not name" invites the wrong conclusion about the crate.

## Item 5: done

`vendor/main/consts-1305.csv` now exists alongside `vendor/main/lua-const-table.tsv`, both
byte-identical to their `work/emu/` originals, and `check-luaconst.sh` reads `vendor/main` with
`RAT_CONSTS` as the override. `work/acquire/main-entrypoints.txt` is still the stale `bl`-only
list and still says nothing about it.
