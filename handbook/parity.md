# Parity — what "exact" means, and how it is proved

The brief is "exact frame data, damage, and range as the character it is normally on", later
sharpened by the user to the form that governs every decision since:

> all moves should have hitboxes accurate to if the character that normally uses them were dropped
> into our modified pikachu's exact circumstances in that moment

That sentence rules out two tempting readings. It is not "the numbers match the source's script",
because a bone-local offset produces a different world position on a smaller skeleton. And it is
not "it looks right", because a hitbox can look right at one frame of a sweep and be absent for
the rest.

## Per axis

| Axis | Standard | Measured by |
|---|---|---|
| **Frames** | the source's script frames, at the source's motion rate | `acmdparity`, `check-acmd-values.sh` |
| **Damage / angle / KBG / BKB** | identical to the source's `ATTACK` arguments | `acmdparity` |
| **Reach** | the hitbox occupies the same **world** position it would on the source, at every active frame | `check-hitboxes.sh`, the FK trace |
| **FAF** | the source's `cancel_frame`, on the source's animation | `mlparity` |
| **Intangibility** | the source's `xlu_start`/`xlu_end` | `mlparity` |
| **Engine calls** | every call the source status makes, the port makes | `check-callparity.sh`, `check-nativecalls.sh` |
| **Params** | every param the source reads resolves to the source's value | `check-srcparams.sh` |

**`check-hitboxes.sh` is the authority on the current numbers, not this page** — the counts moved
every round as moves were added (276 → 372 → 452 hitbox-frames across three snapshots in the
transcript), so quoting one here would go stale by design. What has held constant is the standard:
every traced hitbox-frame within **0.05 world units** of the source, with exactly one recorded
exception — Palutena's n-air, measured worst case 18.53, ceiling pinned at `18.6` in
`check-hitboxes.sh:50` so it cannot grow silently. `tools/moves.tsv` currently carries 52 move
rows; call parity sits at **22 open entries, every one reasoned** — see below on why that number
is not zero and should not be.

## What does not port

**ADR 0001 — character-wide traits do not port.** A property belonging to the source *fighter*
rather than to the move stays behind: K. Rool's belly armor appears across his moveset because he
has a belly, not because his d-air is special. Same for Terry's special-cancel, Cloud's Limit
gauge, and Mythra's Foresight. Move-specific windows — a per-move armor or intangibility window —
do port, because those are per-move script data across the whole cast.

Named exception: **Little Mac's f-smash super armor**, which is arguably Mac-wide but the brief
names it directly.

**ADR 0002 — `sys_*` effects only.** A ported move does not get its source's signature visuals,
because fighter-prefixed effect archives are not resident unless that fighter is loaded, and
referencing one is precisely what creates the CSK dependency the brief forbids. A future reader
will be tempted to "fix" this by wiring up the real effect names; that is a different deliverable,
tracked separately, not a fix to this one.

**ADR 0004 — never hand a victim to another fighter's engine code.** A status being
engine-*common* does not make it fighter-*agnostic*. `status_SwingGaogaenThrown` reads its linked
parent's position with no null check because for Incineroar the parent is always his rope.

## Why call parity is not zero, and why that is correct

`check-callparity.sh` compares the engine calls each source status makes against the calls the
port makes. Three times it has flagged something that is not a defect, always the same way: **it
measures whether a CALL appears, and the port often achieves the same behaviour by a different,
documented mechanism.**

| The port does this instead | Example |
|---|---|
| hardcodes the value as a Rust constant | every param object except `param_special_s` |
| lets the engine read it through `src/params` | the clatter trio — the traced result is identical |
| stores it in `state::` | Incineroar instance work ids that would alias Pikachu's own block |
| substitutes an equivalent at the same frame | Revenge's stance latch, via the ACMD |

The tool is worth having — it found the landing slide, the lariat's rate restore, and Revenge's
animation swap, all real. But **its output is a list of questions, not a list of bugs**, and every
entry has to be read before anything changes. Treating it as a defect list is how the Revenge
regression happened.

The same applies to `check-nativecalls.sh`, which exits 0 on purpose: half of what these statuses
do is presentation this project defers by design. The value is that the list is complete.

## Keeping the checks honest

Every check in this repo has been wrong at least once, in the direction that matters — reporting
clean when it was not. The ones caught, and what caught them:

- **`grep -c … || echo 0` prints `0\n0`**, making a check report ok in both directions. Found by
  self-testing.
- **`printf "$var" | grep -q` under `set -o pipefail`** is a SIGPIPE trap: `grep` exits on match,
  `printf` dies, and the pipeline reports failure. It invented failures nondeterministically in one
  check and would have reported **no gap when there was one** in another.
- **A doc comment naming a call counted as making it.** Two separate checks were gameable by
  prose, so both now strip comments before matching. The honest total went from 57 to 77.
- **Averaging over zero samples**, **matching "no FK skeleton" while the producer printed "no FK
  data"**, **a gate that tested its own output**, and **`blend.rs` passing on zero work** — five
  false cleans in one round, each fixed and then negative-tested.
- **`awk -F'\t'` on a space-aligned file** reported all 25 rows missing. `tools/moves.tsv` is
  tab-*named* and space-*aligned*; every reader uses bare `read -r`.

The rule that came out of it: **a check is not finished until it has been made to fail on purpose.**

## Verification of this handbook's own new data

The lua constant extraction is new, so it is worth stating exactly why it is trusted. Eleven
values this project derived independently — through disassembly, crash reports, and logs, over
several rounds — all reproduce:

| Constant | Derived independently as | Extraction says |
|---|---|---|
| `FIGHTER_STATUS_KIND_CAPTURE_PULLED` | 0x41 | 0x41 |
| `FIGHTER_STATUS_KIND_CAPTURE_WAIT` | 0x42 | 0x42 |
| `FIGHTER_STATUS_KIND_CAPTURE_CUT` | 0x44 | 0x44 |
| `FIGHTER_STATUS_KIND_CAPTURE_JUMP` | 0x45 | 0x45 |
| `FIGHTER_STATUS_KIND_CATCH` | 0x37 | 0x37 |
| `FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED` | 0x176 | 0x176 |
| `FIGHTER_STATUS_KIND_SHOULDERED_DONKEY_START` | 0xC5 | 0xC5 |
| `FIGHTER_PIKACHU_STATUS_KIND_SPECIAL_HI_WARP` | 0x1E1 | 0x1E1 |
| `FIGHTER_INSTANCE_WORK_ID_FLAG_TERM` | 0x200000E0 | 0x200000E0 |
| `LINK_NO_CAPTURE` | table offset 0x4e4 | 0x0 |
| `FIGHTER_GAOGAEN_STATUS_KIND_FINAL_DASH` | table offset 0xb57c | 0x1ED |

And the const table's own signature, which this project derived from `sv_set_status_func` call
sites without knowing the values, holds exactly:

```
0x38    LUA_SCRIPT_STATUS_FUNC_STATUS_PRE      0x0
0x3c    LUA_SCRIPT_STATUS_FUNC_STATUS_MAIN     0x1
0x50    LUA_SCRIPT_STATUS_FUNC_STATUS_END      0x2
0x3074  LUA_SCRIPT_STATUS_FUNC_CHECK_ATTACK    0xC
```

That last row also upgrades an inference to a fact. `reading-the-games-code.md` identified
`[x20, #0x3074]` as CHECK_ATTACK **by elimination** against the decomp's file listing; the table
now names it directly.

Two independent cross-checks on the boundary recovery: the 100,508 recovered entry points sit
against the 99,220 that `module-info-extractor` published for 2.0.0 by the same class of method,
and 1,609 of the indirect call targets the emulator resolved at runtime were already in the list.

## The two standing rules on numbers

**Never invent a number.** If a value cannot be read out of the game, the move is left
unimplemented and the gap is written down. ADR 0005. Guessing costs either immunity or an instant
drop, both worse than an honest absence — that is the exact reasoning that left DK's carry damage
rule unimplemented behind a single unresolved operator polarity.

**A number measured under one condition does not transfer to another.** The estimate that a
decompile would take thirteen hours came from measuring 0.01 s/function on 300 addresses that
already had symbols and applying it to 25,928 that had none. Same shape as the `.bss` crash: 48 MB
was guessed for a region that is sparse, and reading it faulted the game three times.
