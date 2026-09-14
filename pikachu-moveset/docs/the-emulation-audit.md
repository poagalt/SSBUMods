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
by sweeping all 100,508 functions for indirect calls, and `comm` says **all 2,063 are absent from
`all-entrypoints.txt`**. Nobody folded the second round back in. That is 102,571 before a single
extra second of emulation.

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
