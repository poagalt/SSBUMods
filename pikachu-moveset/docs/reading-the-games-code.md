# Reading the game's code

Everything in `vendor/` is gitignored, so this is the versioned copy of what is in there, how it
was obtained and what each source is good for. If `vendor/` is empty, this page rebuilds it.

## The four sources, in order of how much they settle

### 1. `lua2cpp_common.nro` — every status the roster shares

    printf 'prebuilt:/nro/release/lua2cpp_common.nro\n' \
      | tools/arc-pull/target/release/arc-pull.exe <console-ip>:43022 vendor/nro

8466432 bytes, 5428 imported PLT stubs, **8792 defined symbols**. Needs the game running with
the arc_network plugin loaded — the copy on the card is encrypted and decrypting it needs
prod.keys we do not have, so the running game is the only source.

This is where every VICTIM-side status lives. A command grab puts its victim into a common status
— `SWING_GAOGAEN_CATCHED`, `SHOULDERED_DONKEY`, `CAPTURE_PULLED` — and none of those appear in
any fighter's own agent, which is why three rounds of "the grab crashes" could not be explained
from the fighter NROs. See ADR 0004.

### 2. `lua2cpp_<fighter>.nro` — that fighter's own statuses

Same pull, `lua2cpp_gaogaen.nro`, `_donkey`, `_gamewatch`, `_packun`, `_pikachu`. Fighter STATUS
logic is not in `data.arc`'s params, animations or ACMD scripts, and it is not in `main` either;
`main` holds only the engine (`app::lua_bind::*Module__*_impl`). It is in these.

All five pulled builds report `patch-13.0.1` even on a 13.0.5 console, which is why the public
decomp of 13.0.1 matches them exactly.

### 3. The public decomp — the same NROs, as readable C

    git clone --filter=blob:none --no-checkout --depth 1 \
        https://github.com/Coolsonickirby/SSBU-Dumped-Status-Scripts
    git sparse-checkout set ghidra_scripts/lua2cpp_gaogaen.nro ...   # 95 fighters available

Ghidra C output produced by blu-dev's `Lua2CPPAnalysis.java`, which recovers status names by
walking the agent vtable (slot 9 is `SetStatusScripts`). **Fighter agents only — there is no
`lua2cpp_common` in it**, which is exactly the gap source 1 fills.

It also omits every unnamed `FUN_*` helper, and those are where the interesting parts hide. Four
mattered and all four are now read, by source 4.

### 4. `tools/disasm` — the NROs with every call named

    disasm <nro> --sym <substring>      find symbols
    disasm <nro> --calls <addr> [n]     just the call targets, in order
    disasm <nro> <addr> [n]             annotated disassembly
    disasm <nro> --header

Pure Rust (yaxpeax-arm), because w64devkit's objdump has no aarch64 support and there is no
Ghidra on this machine. It walks MOD0 → `.dynamic` → `.rela.plt`, matches each PLT stub to its
JUMP_SLOT relocation, and prints the imported symbol beside every call — which is what makes a
hundred `bl $+0x117e34` lines into something readable.

Addresses map straight through: Ghidra loaded these at `0x7100000000` and `.text` starts at file
offset 0, so `FUN_710001ff80` is file offset `0x1ff80`.

What it answered, each of which had been guessed at wrongly before:

| question | answer |
|---|---|
| how does Incineroar detect the Alolan Whip's catch | he does not. `--sym catch` finds five symbols in the whole NRO and none is `CatchModule`; the engine does it in native code keyed on the fighter kind |
| `FUN_710001ff80` (whip, per-frame) | air control: `is_flag`, `reset_energy`, `controller_set_accel_x_mul/add`, `set_stable_speed` |
| `FUN_710001d4b0` (whip) | the ground/air motion switch |
| `FUN_7100013e50` (Revenge, both statuses) | `get_sum_speed_x/y`, three `get_param_float`, `change_kinetic(UNIQ)`, `clear_unable_energy` twice, `reset_energy`/`set_speed`/`set_accel` on MOTION, `enable_energy` |
| `FUN_7100022520` (DK cargo) | the throw input: `get_clatter_time` on the victim, cat2 masked against four throw flags, `IsThrowStick`, cat1 buttons, five `change_status` |

### Reversing a hash40 back to a name

The disassembly gives 40-bit constants, not names. `tools/motion-patch --bin h40` hashes forward;
to go backwards, hash every token in `vendor/param-labels/ParamLabels.csv` and match. That is how
`special_air_lw_hit_accel_y` was found — a param Revenge reads that nobody knew was involved,
because its name had never appeared anywhere.

## Other people's mods

`vendor/hdr` is HewDraw Remix, the largest smashline gameplay mod, sparse-checked-out to
`fighters/` for the characters this moveset borrows from. It is the best available answer to "is
this how you are supposed to write it": it confirmed the `sub_shift_status_main(L2CValue::Ptr(..))`
idiom, and its `energy/motion.rs` decodes `FighterKineticEnergyMotion` well enough to explain why
`reset_energy` must precede `set_speed`.

**It is a rebalance mod, so its values are not the game's values.** Its `gaogaen`
`special_lw_pre` passes `FIGHTER_STATUS_ATTR_START_TURN` where vanilla passes nothing. The rule
is HDR for *how*, the decomp for *what*.

## The discipline this replaced

Revenge was implemented wrongly five times, the whip's catch three times, and the reason was the
same each time: the logic was being reconstructed from params, ACMD scripts, wiki pages and
reasoning, and **none of those contain it**. Every one of those rounds produced a plausible
implementation and a playtest report saying it was wrong.

So: read the code first. When the code and a wiki disagree, the code wins. When the decomp and
another mod disagree, the decomp wins. When a claim in a comment says "measured", check that a
log actually contains the measurement — one such claim asserted `is_catch` never fires, and every
line of the log it referred to stopped one frame before the window opened.

## Reading an Atmosphere crash report, correctly

**The backtrace walks the FRAME POINTER, not the link register.** That single fact changes how
every report here should be read, and getting it wrong cost most of a round.

A function that never establishes a frame — no `stp x29, x30, [sp, #n]` — does not appear in the
backtrace at all. Rust release builds are full of those: leaf functions, outlined prologues and
epilogues (`mov x1, x30; bl OUTLINED; mov x30, x1`), and anything LLVM decided to tail-call.

So `ReturnAddress[00]` is not necessarily the caller of the faulting code. It is the nearest
return address belonging to a function that kept a frame. Worked example, the cargo crash:

```
Instruction Abort, Address 0x3a00000000
  ReturnAddress[00]: rat_of_doom + 0x9cf8
  ReturnAddress[01]: smashline + 0x2ca5c
```

Read naively that says our code at `0x9cf4` branched to garbage. It does not. Disassembling
`L2CFighterCommon::sub_shift_status_main` out of `lua2cpp_common.nro` shows it ends with

```
bl   L2CFighterBase::shift
bl   L2CValue::as_pointer
mov  x9, x0
blr  x9                      <- calls the status main loop
```

a real `blr`, and it does establish a frame (`x29 = sp + 0x50`, saving x29/x30 at `[sp+0x50]`).
The saved x30 there is *its own* caller's return address — `0x9cf8`, in our plugin. So a fault
inside `blr x9`, in a loop function with no frame of its own, reports `0x9cf8` as frame 0 and
omits the two frames in between.

The lesson: **`ReturnAddress[00]` plus the faulting ADDRESS TYPE is the real signal.** A Data
Abort with a small fault address is a null field read and the top frame usually is the culprit.
An Instruction Abort is a branch to an address that is not code, and the top frame is whatever
still had a frame pointer — which may be several calls above.

### Turning an offset into a function

Rust strips almost everything: `librat_of_doom.nro` has 175 imported PLT stubs and 5 defined
symbols. So identify our own code by its STRINGS, not its symbols. `disasm <nro> <addr> 60`
around the offset, find the `adr xN, $+...` that points past `.text`, and read that offset out of
the file:

```bash
dd if=librat_of_doom.nro bs=1 skip=$((0x1af3d)) count=120 | tr -c '[:print:]\n' '.'
```

A format string is unique to one function, which identifies it exactly.

**And check the build.** `rat_of_doom + 0x9cf8` means nothing without knowing which build. The
`.nro` was 159744 bytes when that report was written and 167936 bytes two commits later; the
layout moved. `analyze.sh` compares the log's BUILD stamp against HEAD for the same reason.

## The four unnamed helpers, and the fifth question

`tools/pull-nro.sh` was written to answer four questions the public decomp could not. All four are
now answered, and so is the one it listed as most important:

**Does Incineroar's SPECIAL_LW register a CHECK_DAMAGE line?** No. Nothing does.

`FUN_7100000e70` is his whole status registration — 53 `sv_set_status_func` calls — and it uses
exactly four condition constants:

| condition | count | what |
|---|---|---|
| `[x20, #0x38]` | 16 | one per status |
| `[x20, #0x3c]` | 16 | one per status |
| `[x20, #0x50]` | 15 | one per status |
| `[x20, #0x3074]` | 6 | **six statuses only** |

Sixteen statuses × three lines is pre / main / end, and the six outliers are identified by
elimination against the decomp: the only Incineroar statuses with a file beyond
pre/main/main_loop/end are the six `SpecialHi*` with `check_atk.c`, plus the Final Smash's
init/exit. So `[x20, #0x3074]` is CHECK_ATTACK, registered for his up-B and nothing else.

**Revenge registers pre, main and end. That is all it has.** Which settles a question this
project kept re-opening: the counter's damage detection is not a status line we failed to find,
it is native C++ in `main` — the same place the Alolan Whip's catch turned out to live. A frame
callback reading `DamageModule::damage` deltas is not a workaround for missing the right hook. It
is the only hook there is.

Worth knowing for next time, from smashline's own `install_status_scripts`: it supports
seventeen lines, not three — Pre, Main, End, Init, Exec, ExecStop, Post, Exit, MapCorrection,
FixCamera, FixPosSlow, **CheckDamage**, **CheckAttack**, OnChangeLr, LeaveStop,
NotifyEventGimmick, CalcParam. So CheckDamage *is* available to us even though Incineroar does
not use it, which makes it a design option for the whip being hit out of its lunge rather than a
thing to go looking for in the source again.

## Two more disasm modes, and what they found

    disasm <nro> --xrefs <addr>          every bl/b whose target is that address, with the
                                         containing function named
    disasm <nro> --scan <substr> [n]     every instruction in .text matching a substring, paired
                                         with the next NAMED call within n instructions

`--xrefs` answers "who calls this engine function, and therefore with what arguments" — which is
the only honest way to use one whose parameters are unnamed integers. `--scan` exists because the
plain disassembly and `--calls` both stop at the first `ret`, which makes them useless for a
whole-module sweep.

### `CaptureModule::capture`, partially settled

This is the function that would let a hand-made command grab hold its victim the way the engine
does — inert, positioned, escapable — and its signature is
`(boma, u32, i32, bool, i32)`, four unnamed integers.

`--xrefs` on its PLT stub in `lua2cpp_common.nro` finds exactly two call sites, both in
`L2CFighterCommon::status_MiifighterCounterThrown`, and reading back from them gives:

```
w1 = LinkModule::get_parent_id(boma, <lua_const + 0x4e4>)   the CAPTURER's object id
w2 = <lua_const + 0x250>                                    unidentified
w3 = true
w4 = 0
CaptureModule::capture(victim_boma, capturer_id, <0x250>, true, 0)
```

So it is called **on the victim**, with the capturer's battle object id — which we have as
`(*boma).battle_object_id` — and `true, 0` for the last two. Two of the four are now facts.

`<lua_const + 0x250>` is not recoverable this way. The lua_const globals are one array in `main`
filled at runtime, so the NRO holds only offsets, and the only uses of 0x250 in the common module
are this one and unrelated loads through other base registers. It is presumably a capture node or
kind; presumably is not good enough (ADR 0005), so this is written down rather than attempted.

**It is not needed yet.** The reported symptom was the victim falling off, which was ownership
(ADR 0004's sibling problem) and is fixed. If a future playtest shows the held victim *acting* —
attacking or moving while carried — this is the route, and the missing argument can be settled by
logging `FIGHTER_STATUS_CAPTURE_*` candidate constants at runtime and matching the value.

## Source 5: `main` itself, and source 6: the public matching decompilation

### `main`, out of the running process

`data.arc` does not contain fighter status LOGIC and neither do the lua2cpp NROs. "When this catch
connects, put the victim there and me here" is compiled C++ in `main`, the title's ExeFS NSO,
keyed on the fighter kind. That is the single reason six of these moves are reimplementations
rather than drag-and-drops.

It needs no keys. A Skyline plugin is injected INTO the game process, so `main` is mapped memory:
`src/telemetry/exedump.rs` reads `getRegionAddress` for each region, walks MOD0 -> `.dynamic` ->
`.dynsym` exactly as `tools/disasm` does on an NRO, and writes the names and the bytes to the
card. `tools/pull-exedump.sh` brings them back. Flip `ENABLED` in that file, load one match, pull.

What arrives: `.text` 60592128 bytes, `.rodata`, `.data`, a 48 MB slice of `.bss`, and 4,917
symbol names. `disasm --flat` reads a raw region against an external symbol table.

**The export list is not the function list.** 4,917 names against ~39,600 functions, and they are
all exports: 2,225 `app::lua_bind::*`, 248 `app::ai`, 219 `sv_animcmd`. `FighterGaogaen`,
`FighterDonkey` and `FighterGamewatch` return ZERO hits, because internal statics are not
exported. So `--xrefs` rather than `--sym`: 309 `FighterSpecializer_*` symbols ARE exported and
are called from the fighter-specific code, so working backwards from one lands inside it.

That is how the Alolan Whip got answered. `FighterSpecializer_Gaogaen::generate_rope` at
`0xabe800` has one caller in `main`, a lua-binding thunk -- so the rope is made from the STATUS
SCRIPT layer, Incineroar's own NRO imports it, and the decomp names the call site as
`SpecialSThrow/main.c:23`.

`.rodata` and `.data` are in the dump for the VTABLES, which are how an unexported class gets
identified at all, and `.data`/`.bss` for the LUA_CONST ARRAY -- one global i32 array every NRO
reads its constants from. An NRO only shows offsets (`ldr w1, [x24, #0x250]`), which is why
`CaptureModule::capture`'s third argument stayed unnamed; with the array dumped an offset becomes
a value, and smash-rs already holds ~19,000 name-to-value pairs. The array is findable by
signature: offsets 0x38, 0x3c and 0x50 must read 0, 1 and 2, with 0xC at 0x3074.

### The public matching decompilation — check for this FIRST next time

`https://github.com/sbergeron42/ssbu-decomp`, targeting patch 13.0.4. See
`vendor/ssbu-decomp/README-rat.md`. `data/ghidra_1301_named.csv` is **15,320 named functions for
13.0.1**, including the internals our own `.dynsym` does not have, and `data/functions.csv` is all
39,636 with sizes. `include/` carries decompiled headers with vtable indices.

Merging it with our export table gives `vendor/main/symbols-merged.txt`, and that combination
immediately settled `CaptureModule::Capture` -- which two rounds had guessed at and one had
explicitly declined to guess at.

**The lesson is procedural.** Six rounds were spent reconstructing logic that a public
decompilation already had a map for, and nobody searched for it. Look for the community artifact
before building the tool: the tool was still worth building, but it would have been aimed better.

---

## 5. `vendor/ssbu-status/lua_dict.txt` — the constant table, which unblocked everything else

19,310 `offset:NAME` lines, and it was in the tree for rounds before anyone read it.

**The problem it solves.** Every fighter's compiled status code reads its constants out of ONE i32
array — `lua2cpp::L2CAgentGeneratedBase::const_value_table__` — so a status kind, a work id, a
flag and a link number all look the same in a disassembly:

    ldr w1, [x23, #0x4e4]

and carry no name at all. `CaptureModule::capture`'s third argument was `lua_const[0x250]` for this
whole project, and chasing it is what the executable dump existed for — three crashes' worth.

    0x250   STATUS_KIND_NONE
    0x4e4   LINK_NO_CAPTURE
    0x9cc   FIGHTER_STATUS_KIND_CAPTURE_CUT

`tools/disasm` now annotates every `ldr wN, [xM, #off]` with the name, marked `~lua_const` because
it is a hint: any struct field read at the same offset gets a name it has nothing to do with.

**Three cross-checks, so it is trusted for a reason.** `0x38` is
LUA_SCRIPT_STATUS_FUNC_STATUS_PRE, which matches the anchor derived independently from
`sv_set_status_func` call sites. `0xb57c` is FIGHTER_GAOGAEN_STATUS_KIND_FINAL_DASH, which is
exactly the first constant the decompiled `create_agent_fighter_status_script_gaogaen` names.
`0x4e4` is LINK_NO_CAPTURE, which matches the link number derived from `SpecialSThrow`'s
`send_event_nodes` calls.

**Two dead ends, recorded so nobody pays for them twice.** The table is filled at load by
`construct_bind_const_value_table`, which walks 19,469 precomputed 64-bit hashes in
lua2cpp_common's `.rodata` and looks each one up. Inverting that hash does not work: main +
0x399f660 loads all five XXH64 primes and dispatches on `cmp len, #0x20`, but neither XXH3-64 nor
XXH64 with seed 0 reproduces a single one of the 19,469 values, and the registrar hashes
`"<namespace>:<name>"` with a 0x3a separator rather than the bare name. Neither does ORDER: main
registers 40,338 constants by name — `luaconst --order` extracts that whole sequence — while the
table holds a 19,469-entry subset, so index N is not registration N.

## 6. hash40, from three sources, behind one tool

    bash tools/hash40.sh 0x8af9b3d1d      -> shoulder
    bash tools/hash40.sh shoulder         -> 0x8af9b3d1d
    bash tools/hash40.sh --len 19         every known 19-character name

The other half of what status code passes around is a hash40 and nothing else.
`LinkModule::send_event_nodes(boma, LINK_NO_CAPTURE, 0x8af9b3d1d, 0)` is how Donkey Kong tells the
fighter on his shoulder to be carried, and the call cannot be reproduced until that constant has a
name.

    vendor/Hashes.txt + the prc labels   149,871 community names: paths, motions, params
    the dumped vanilla ACMD scripts      every `Hash40::new("...")` literal, 14,629 names
    vendor/main/hash40-dict.txt          47,856 hashes of every identifier-like string in main's
                                         .rodata and .text, from `luaconst --hashdict`

The third is the one nothing else replaces. Donkey Kong's carry `check_dmg` reads two keys out of
the damage table and they are `owner_object_id_` and `reaction_max_` — C++ member names with
trailing underscores, exposed into an L2C table. No community list has those.

A hash in none of the three was baked by the L2C compiler from a string that never reaches the
executable or the archive. Those are guessable, because the naming is regular and a guess is
CONFIRMED by hash rather than argued for — `--len` narrows it, since hash40 carries the length in
its top byte. `gaogaen_swing_throw` fell on the first attempt that way.
`vendor/main/hash40-confirmed.txt` records each one with the constant it was matched against.

## 7. Three `tools/disasm` modes worth knowing about

    --got [slot]        what global an `adrp`+`ldr` pair reads, by relocation symbol. This is what
                        named `const_value_table__` and made section 5 possible. Slot addresses are
                        MODULE-RELATIVE: the pair at Ghidra 0x710052d8e0 reads slot 0x80b578.
    --dataxref <addr>   who references a data address, by tracking `adrp` pages and matching the
                        `add`/`ldr` that completes the pair. How a string in `.rodata` becomes the
                        function that uses it.
    --flat <text.bin>   a raw image plus an external symbol table, for `main`. Pair it with
                        RAT_SYMS=vendor/main/symbols-merged.txt.

## 8. `tools/check-nativecalls.sh` — the gap list, kept honest

Compares the engine calls each source status makes against the calls this port makes, across all 21
ported statuses. Every grab bug this month was a call the port simply did not make — not a wrong
value, not a wrong status graph, a missing line — and each was found by reading one decompiled
function closely AFTER the symptom appeared. This is that reading done mechanically, in advance.

It exits 0 on purpose: a missing call can be entirely correct, since half of what these statuses do
is presentation this project defers by design. The value is that the list is complete.

It strips comments before matching, because the first thing it caught was itself: a doc comment
explaining why a call is not made names that call, and a plain text search counts it as made. The
same lesson `tools/check-srcparams.sh` learned with `NOT MODELLED:`.

## 9. `disasm --trace` — the decompilation lua2cpp_common does not have

    tools/disasm/target/release/disasm.exe vendor/nro/.../lua2cpp_common.nro \
      --trace 0x710028a510 1400

`vendor/ssbu-status` ships Ghidra-decompiled C for five FIGHTER NROs and not for `lua2cpp_common`
— and lua2cpp_common is where the VICTIM side of every grab lives. `SwingGaogaenCatched`,
`CaptureWait`, `CapturePulled`, `MiifighterCounterThrown`: all of it was read one `--calls`
invocation at a time with the constants cross-referenced by hand, which is why each grab bug cost
a full investigation. There is no Ghidra on this machine, so this is the substitute.

For a status script it is most of what a decompilation would give, because an L2C status function
IS a straight sequence of engine calls with constants marshalled in between:

    call LinkModule__set_model_constraint_pos_ort  [LINK_NO_CAPTURE, "rot", "throw",
                                                    CONSTRAINT_FLAG_POSITION,
                                                    CONSTRAINT_FLAG_ORIENTATION]
    call WorkModule__get_param_int    ["param_special_s", "special_s_catch_clatter_frame"]
    call ControlModule__start_clatter

The L2CValue constructor and accessor traffic is dropped — 45 lines of signal instead of 400 of
noise — and constants accumulate across the dropped calls and attach to the next real one, which
is what the marshalling is for. Names come from the module's symbols, `lua_dict.txt`, and the
hash40 name list, so `0x8af9b3d1d` prints as `"shoulder"`.

It paid for itself the first time it ran. `set_model_constraint_pos_ort` turns out to bind the
victim to the captor's **`"throw"` bone** with **`"rot"`** — the grip, free, the moment a capture
exists. And four lines of `CaptureWaitCommon` settle how a victim escapes:

    call ControlModule__get_clatter_time
      tbz ...
    call WorkModule__is_enable_transition_term  [FIGHTER_STATUS_TRANSITION_TERM_ID_CAPTURE_CUT]
    call L2CFighterBase::change_status          [FIGHTER_STATUS_KIND_CAPTURE_CUT]

so the mash-out is a clatter timer gated by a transition term that the victim's own
`status_CaptureWait` enables. The capture statuses are 0x41 PULLED, 0x42 WAIT, 0x44 CUT, 0x45 JUMP
— worth knowing by number, because that is how they appear in `crate::victim::trace`.

## What is still missing, as of this writing

Written down because a known gap is cheaper than a rediscovered one.

- **The status-line dispatcher's return-value polarity.** What `main` does with the value a
  `CheckDamage` line returns. Blocks DK's carry damage rule; see BACKLOG.md, where the whole
  decompiled rule is recorded waiting on this one fact. The way in is the native caller of the
  line, through `FighterStatusModuleImpl`'s vtable.
- **What the engine requires of a status for it to keep a capture alive.** Four of Donkey Kong's
  `init_settings` and `set_fighter_status_data` arguments were not being copied, and
  FS_SUCCEEDS_KEEP_ATTACK_ABSOLUTE is the leading candidate, but the mechanism has been inferred
  from the argument's name rather than read. If there is a per-status attribute table in `main`,
  reading it would answer this and several other questions at once.
- **Why `CaptureModule::capture` returned without capturing.** `is_capture` was false on the frame
  after the call, with `catch_object_id` still BATTLE_OBJECT_ID_INVALID. The suspicion is that
  `capture` updates bookkeeping for a link the CATCH system creates, rather than creating one —
  every native caller invokes it on a victim that is already linked. If that is right, the missing
  call is on the captor's side: `CatchModule::set_catch`.
- **`main`'s `.data` and `.bss`.** No longer wanted. `lua_dict.txt` answered the question they were
  being dumped for, and dumping them during play crashed the game three times.
