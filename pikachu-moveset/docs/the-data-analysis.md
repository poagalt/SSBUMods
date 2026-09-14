# What the new data says, and what it changes

Written after the acquisition round: `lua2cpp_common.nro` decompiled in full, `main` at 8,904
functions, 96 of 97 fighter NROs, 95 fighters' status scripts where there had been five, every
fighter's params, motion lists and skeletons, and a map of all 590,711 files in the archive.

## The headline is a negative result, and it is the valuable one

**No ported move has source status logic this port has not implemented.** Of the 52 rows in
`tools/moves.tsv`, exactly 12 have a status directory in their source fighter's tree -- and all
12 are moves already implemented as hand-written status modules (the whip's six, Revenge's two,
the four cargo throws, G&W's up-B).

The other 40 -- every normal, aerial, smash, the ported throws, the get-up attacks -- have **no
status code in their source fighter at all**. Their behaviour is entirely ACMD plus animation,
which is what ADR 0003 ports and what `check-hitboxes.sh` and `check-acmd-values.sh` already
gate.

That could not be established before. With five fighters' trees it was an assumption about the
other fifteen; with 95 it is a checked fact. It means the exactness work is bounded: it lives
entirely in four hand-written modules, not spread across 52 moves.

## What the motion lists confirmed, by already being right

`mlparity.exe` compares every ported motion's FAF, flags, blend and animation against the
source's, and it has been running in `tools/check.sh` all along. With every fighter's
motion_list now local it still passes -- and it is why my FAF theory for the Revenge autocancel
was wrong. `special_lw` is faf 27 and `special_lw_start` is faf 59 on Pikachu **and** on
Incineroar, because `tools/motion-patch` copies them.

A check that was already green is not a finding, but it is worth saying plainly: the new data
did not reveal a frame-data gap, and I should have read the check before inventing a cause.

## What the params confirmed, also by already being right

The ported source statuses read four param objects: `param_special_s`, `param_special_lw`,
`param_special_hi` and `param_private`. `src/params` answers only `param_special_s` at runtime,
which looked like a gap and is not one -- the others are hardcoded in Rust and
`check-srcparams.sh` verifies them against the same prc files. For gamewatch it reports **5 used,
0 declared unmodelled, 0 unaccounted**: `stick_min 0.25`, `stick_mul 30`, `limit_y_mul 0.5`,
`landing_frame 40`, `pass_speed_mul 1.1`.

### The lesson that keeps recurring

Three times now `check-callparity.sh` has flagged something that is not a defect, and always the
same way: **it measures whether a CALL appears, and the port often achieves the same behaviour by
a different, documented mechanism.**

| the port does this instead | example |
|---|---|
| hardcodes the value as a Rust constant | every param object except `param_special_s` |
| lets the ENGINE read it through `src/params` | the clatter trio -- traced result is identical |
| stores it in `state::` | gaogaen INSTANCE work ids that would alias Pikachu's own block |
| substitutes an equivalent at the same frame | Revenge's STANCE latch, via the ACMD |

The tool is still worth having -- it found the landing slide, the lariat's rate restore and
Revenge's animation swap, all real. But its output is a list of questions, not a list of bugs,
and every entry has to be read before anything is changed. Treating it as a defect list is how
the Revenge regression happened.

## The engine's ceiling is liftable

I reported that `main` stops at 8,905 functions because no public symbol map exists for 13.0.5,
and that this was a hard ceiling. It is not.

`ultimate-research/module-info-extractor` publishes `main_200_function_list.txt`: **99,220
function start addresses**, recovered by emulating the module with Unicorn rather than by having
symbols. It is addresses only -- no names -- and it is for version 2.0.0, so the addresses do not
transfer to 13.0.5.

What transfers is the METHOD. Ghidra with `-noanalysis` cannot find a function it has no symbol
for, which is exactly why the engine decomp covers 8,905 of an estimated ~99,000. Running the
same emulation against this project's 13.0.5 `main` would produce the boundary list Ghidra is
missing, and `decomp-all.sh` already decompiles any address list it is handed. The result would
be unnamed -- `FUN_xxxxxxxx` -- but decompiled bodies are greppable by content, which is how
`FUN_7100021af0` was read in the first place.

Cost estimate: the emulator run is minutes; the decompilation is ~99,000 functions against the
8,904 that took seven minutes, so roughly thirteen hours, resumable. Not attempted yet.

## What else the archive map says

590,711 files. Of the 431,971 under `fighter/`, the gameplay-bearing ones are now complete
locally: **9,243 / 9,243 params, 5,660 / 5,660 motion lists, 7,208 / 7,208 skeletons.** The
280,161 animations are 36,352 local, and the rest are textures, models and materials with no
gameplay content.

The animations are the one remaining bulk gap, and per-file network pulling is the wrong tool for
them -- at the measured rate it is days. They want a single archive dump and a local extract.
Nothing currently blocks on them: the moveset ports 77 motions and has all of them.

## Reference data now local

`tools/fetch-refs.sh` clones what the community publishes and this project was working without:

* **param-labels** -- `ParamLabels.csv`, hash to name for every param field. This is what turned
  three anonymous hashes in G&W's up-B into `stick_min`, `stick_mul` and `landing_frame` in one
  grep, and it should be the first stop for any unknown param hash from now on.
* **SSBU-Dumped-Motion-Lists** -- every vanilla motion_list, for cross-checking `mlparity`.
* **prc-rs / motion_lib** -- the Rust readers for both formats.
