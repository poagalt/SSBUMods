# Where the truth lives

Nine sources, in the order of how much they settle. Everything under `vendor/` and `work/` is
gitignored by design — the repo versions the recipe, not Nintendo's bytes — so this page is how
either directory gets rebuilt.

The console is a modded Switch running Smash **13.0.5** at `<console-ip>`. Every pull that needs
the game running needs it running with the `arc_network` plugin loaded, because the copy of
`data.arc` on the card is encrypted and decrypting it needs `prod.keys` this machine does not
have. The running game is the only source.

---

## 1. `lua2cpp_common.nro` — every status the roster shares

```bash
printf 'prebuilt:/nro/release/lua2cpp_common.nro\n' \
  | tools/arc-pull/target/release/arc-pull.exe <console-ip>:43022 vendor/nro
```

8,466,432 bytes, 5,428 imported PLT stubs, **8,792 defined symbols**.

This is where every **victim-side** status lives. A command grab puts its victim into a common
status — `SWING_GAOGAEN_CATCHED`, `SHOULDERED_DONKEY`, `CAPTURE_PULLED` — and none of those appear
in any fighter's own agent, which is why three rounds of "the grab crashes" could not be explained
from the fighter NROs at all.

## 2. `lua2cpp_<fighter>.nro` — that fighter's own statuses

Same pull, per fighter. **20 are local** (`vendor/nro/prebuilt/nro/release/`), not the whole
roster — and that is sufficient, because source 3 covers 95 fighters as readable C. Pull an NRO
when you need something the decomp omits: an unnamed `FUN_*` helper, or `lua2cpp_common`, which
has **no** decomp at all and is where every victim-side status lives.

All of them report `patch-13.0.1` even on a 13.0.5 console, which is why the public 13.0.1 decomp
matches them exactly — no version drift to reason about.

## 3. The public status decomp — the same NROs as readable C

```bash
bash tools/fetch-status-scripts.sh      # 95 fighters, 124 MB, vendor/ssbu-status-all/
```

`Coolsonickirby/SSBU-Dumped-Status-Scripts`: Ghidra C produced by blu-dev's
`Lua2CPPAnalysis.java`, which recovers status names by walking the agent vtable (slot 9 is
`SetStatusScripts`).

**Read the directory listing before reading any code.** Under
`ghidra_scripts/lua2cpp_<f>.nro/<f>/<f>/status/<Status>/` there is one `.c` per line the status
actually has, so `ls` answers "does this status have an Exec line" for free. That is cheaper and
more reliable than disassembly, and skipping it cost a round.

Two limits: there is **no `lua2cpp_common`** in it (source 1 fills that), and it omits every
unnamed `FUN_*` helper — which is where the interesting parts hide.

## 4. `main` itself — the engine

A Skyline plugin is injected *into* the game process, so `main` is mapped memory and needs no
keys. `src/telemetry/exedump.rs` reads `getRegionAddress` per region, walks MOD0 → `.dynamic` →
`.dynsym`, and writes bytes and names to the card; `tools/pull-exedump.sh` brings them back. Flip
`ENABLED` and `DUMP_TEXT` in that file, load one match, pull, then flip them back — the dump
stalls a frame for several seconds and writes 60 MB.

What arrives in `vendor/main/`:

| File | Size | Note |
|---|---|---|
| `rat-main-text.bin` | 60,592,128 | `.text`, module-relative from 0 |
| `rat-main-rodata.bin` | 22,339,584 | strings, jump tables, and the precomputed const hashes |
| `rat-main-data.bin` | 3,747,840 | **already relocated** — see below |
| `rat-main-bss.bin` | 6,815,744 | a prefix; `.bss` is sparse and a linear read faults |
| `rat-main-symbols.txt` | 4,917 names | the **export** list, not the function list |
| `symbols-merged.txt` | 8,906 names | exports merged with the public decomp's names |

Bases are in `rat-exedump.txt` and `rat-memserve.txt`. **The `.data` and `.bss` copies come out of
a running process, so every pointer in them is already relocated for that run's base of
`0x4dd3c04000`.** That single fact is what makes static analysis of vtables possible without a
linker, and it is what the Unicorn work (source 9) is built on.

**The export list is not the function list.** 4,917 names against ~99,000 functions, and they are
all exports: 2,225 `app::lua_bind::*`, 248 `app::ai`, 219 `sv_animcmd`. `FighterGaogaen`,
`FighterDonkey`, and `FighterGamewatch` return zero hits, because internal statics are not
exported. So the technique is **cross-referencing, not lookup**: 309 `FighterSpecializer_*`
symbols *are* exported and are called from the fighter-specific code, so working backwards from
one lands inside the function you wanted. That is how the Alolan Whip was answered —
`FighterSpecializer_Gaogaen::generate_rope` at `0xabe800` has exactly one caller, a lua-binding
thunk, which proves the rope is created from the status-script layer.

## 5. The public matching decompilation — check for this FIRST next time

`sbergeron42/ssbu-decomp`, targeting patch 13.0.4, in `vendor/ssbu-decomp/`:
`data/ghidra_1301_named.csv` is **15,320 named functions** including internals our own `.dynsym`
does not have, `data/functions.csv` is all 39,636 with sizes, and `include/` carries decompiled
headers with vtable indices. Merging it with the export table produces
`vendor/main/symbols-merged.txt`, and that combination immediately settled
`CaptureModule::Capture`, which two rounds had guessed at.

**The lesson is procedural.** Six rounds went into reconstructing logic a public decompilation
already had a map for, and nobody searched for it. Look for the community artifact before building
the tool.

## 6. `data.arc` — params, animations, motion lists, skeletons, ACMD

```bash
bash tools/arc-fetch.sh <path-list>      # over arc_network, game running
```

590,711 paths, 590,707 of them named. Gameplay-bearing extraction is complete locally:
**9,243/9,243 params, 5,660/5,660 motion lists, 7,208/7,208 skeletons**, plus one full `c00`
fighter extraction (40,264 files, 1.27 GiB, 0 failures). The 280,161 animations are 36,352 local
and the rest are textures, models, and materials with no gameplay content; nothing currently
blocks on them.

A full `data.arc` pull was attempted and abandoned at 0.59 GiB. Measured throughput: FTP
446 KB/s, MTP over USB ~40 MB/s, arc-network ~0.8 s per small file marginal, ~97 files/s with the
512 KiB block cache in `tools/arc-pull/src/block_reader.rs`.

## 7. The vanilla ACMD dumps

`WuBoytH/SSBU-Dumped-Scripts` (309 MB) — every fighter, weapon article, and item. Better than a
frame-data site for armor and intangibility windows, because it is the actual script rather than a
derived table. `vendor/refs/` adds `param-labels` (`ParamLabels.csv`, hash → name for every param
field), `SSBU-Dumped-Motion-Lists`, `prc-rs`, and `motion_lib`, all via
`bash tools/fetch-refs.sh`.

`ParamLabels.csv` should be the **first stop for any unknown param hash**. It turned three
anonymous hashes in G&W's up-B into `stick_min`, `stick_mul`, and `landing_frame` in one grep.

## 8. The console, played on purpose

Some facts are only observable. `src/telemetry/` hooks sixteen engine functions and writes a log
to the SD card; `tools/watch.sh` and `tools/analyze.sh` pull and summarise it. Playing the
**source fighter** natively with the tracer installed produces
`pikachu-moveset/docs/the-recording.md` — 533 traced calls that outrank reasoning anywhere else in
this repo.

Two rules paid for in full:

- **A Skyline plugin loads at game launch.** Deploying to a running game changes nothing, and a
  whole recording session was wasted because the instruction "restart Smash" was never given.
  `analyze.sh` now compares the log's build stamp against HEAD for exactly that reason.
- **A tracer's silence is not evidence.** "The early-press branch isn't in the recording" was
  wrong: the hooks covered capture-state calls only, and that branch touches none of them.

## 9. Unicorn emulation of `main` — `tools/emu-module.py`

The method comes from `ultimate-research/module-info-extractor`, which emulated 2.0.0's module
initialisation and read its registration calls as they happened. Its addresses are 2.0.0 and do
not transfer; the method does. Install `unicorn` and `capstone` into the Python 3.12 at
`%LOCALAPPDATA%\Programs\Python\Python312`, then:

```bash
python tools/emu-module.py ptrs      # function pointers out of .data/.bss     ~30 s
python tools/emu-module.py find      # locate the registration call site        ~1 s
python tools/emu-module.py consts    # every lua constant, name and value       ~2 s
python tools/emu-module.py indirect  # emulate every entry point, log blr/br   ~10 min
```

What it settles, and how:

- **Function boundaries.** `.data` holds 289,768 relocated pointers into `.text`, 70,143 of them
  distinct, **67,651 not reachable as a `bl` target** — vtable slots and function-pointer tables,
  which is exactly the documented ceiling on `tools/disasm --callees`. Union with the 32,857
  `bl`-derived entry points: **100,508**, against the ~99,220 that 2.0.0's published list contains.
  No emulation is needed for this at all; the pointers are already relocated (source 4).
- **The constant table.** 13.0.5 registers its lua constants from **55 functions**, not the one
  2.0.0 hardcoded; `find` locates the register function at `0x37289c0` by taking a known constant
  name string in `.rodata` and scanning `.text` for the `adrp`/`add` pair that materialises it,
  then attributing its 40,338 call sites to their owners. Emulating those 55 bodies yields
  **39,213 distinct constants with names and values**, which joined against the offset dictionary
  resolves `const_value_table__` completely.
- **Indirect call edges, and a cross-check.** The `indirect` sweep emulates each of the 100,508
  entry points under a 20,000-instruction budget and records every `blr`/`br` target it resolves.
  Yield is modest by construction: most vtable dispatches load through a `this` pointer the sweep
  does not have, so they fault before dispatching. Of the 1,025 addresses it resolved as **call**
  targets, **994 — 97% — were already in the list**, which is the useful result: an independent
  runtime check on a statically derived boundary set. Net new boundaries: 31. Keep the `blr` lines
  and discard the `br` ones, which are mostly switch-table labels.

Two implementation notes that are the difference between working and not:

- **Map every segment at `0x4dd3c04000`**, the base the dumped `.data` was relocated for. Then the
  module's own vtables and function tables resolve with no linker. `.rodata` was measured to hold
  no absolute pointers at all (one byte-aligned false positive in 22 MB), so mixing it in from an
  earlier dump with a different base is safe.
- **Emulate a function's own body and skip every call.** Running the registering function for real
  means the first allocator call returns garbage, a vtable dispatch off that garbage jumps to an
  address that is not code, and the run dies 1,107 entries into a list of 3,338. Skipping is sound
  *here specifically* because the registration arguments are built in the registering function
  itself — the name from an `adrp`/`add` against `.rodata`, the value from an immediate — and never
  come back from a callee. Install the code hook with `begin == end == the register address` so
  Unicorn calls into Python 40,338 times instead of once per instruction; that is minutes versus a
  day.

---

## Precedence, restated as a rule

The recording beats the code beats the data beats another mod beats a wiki. Within the code:
`main` for engine behaviour, the fighter NRO for status logic, `lua2cpp_common` for anything
happening to a victim. And **when a source cannot answer, say so and write it down** — ADR 0005
forbids a fix without evidence, and [open-questions.md](open-questions.md) is where the gaps go.
