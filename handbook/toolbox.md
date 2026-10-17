# The toolbox, as questions

Every tool in `tools/`, indexed by the question it answers. Each script's own header is the
authority on its usage; this page is for finding the right one.

## "What does this code do?"

| Tool | Question |
|---|---|
| `disasm <nro> <addr> [n]` | annotated AArch64, with every `bl` named from `.rela.plt` and every `ldr wN,[xM,#off]` annotated `~lua_const` |
| `disasm --sym <substr>` | which symbols exist in this module |
| `disasm --calls <addr> [n]` | just the call targets, in order — the shape of a function in ten lines |
| `disasm --trace <addr> [n]` | **the decompilation `lua2cpp_common` does not have.** Drops the L2CValue marshalling and accumulates constants onto the next real call: 45 lines of signal instead of 400 of noise |
| `disasm --xrefs <addr>` | who calls this, and therefore with what arguments — the only honest way to use a function whose parameters are unnamed integers |
| `disasm --dataxref <addr>` | who references this data address, by tracking `adrp` pages. How a `.rodata` string becomes the function that uses it |
| `disasm --got [slot]` | what global an `adrp`+`ldr` pair reads, by relocation symbol. This is what named `const_value_table__` |
| `disasm --scan <substr> [n]` | every matching instruction in `.text`, paired with the next named call. Exists because plain disassembly stops at the first `ret` |
| `disasm --statusmap` | the status-kind → function map, from `sv_set_status_func` call sites |
| `ssbu.py find <anything>` | **start here**: everything every index and doc knows about a name, address, hash or value. See [lookup.md](lookup.md) |
| `code-index.py fn\|callers\|refs-to\|strings` | the game's code without opening Ghidra, data references included |
| `move-index.py move <f> <motion>` | one move: animation, frames, hitboxes, effects with colours, sounds, articles, statuses, what is missing |
| REA (MCP server `rea`) | **who calls this, what uses this string, where does this argument come from**, followed hop by hop in one live Ghidra or Hopper session. Opens `work/rea/*.elf` by Linux path. See [rea.md](rea.md) |
| `rea-elf.py main\|nro <m>\|all` | wraps a module as an AArch64 ELF REA can open, named from its own `.dynsym`/`.rela` |
| `rea-wsl.sh setup\|update\|check` | installs, updates, or checks REA and its engines in WSL |
| `disasm --callees` | every `bl` target in the image: 25,928 distinct targets from 537,516 `bl` instructions, plus 6,929 symbols nothing calls = **32,857 entry points**, with no emulation and no ELF conversion |
| `disasm --flat <bin>` | a raw region plus an external symbol table, for `main`. Pair with `RAT_SYMS=vendor/main/symbols-merged.txt` |
| `decomp.sh <module> <addr>` | one function as annotated C, with lua_const reads and hash40 immediates resolved |
| `decomp-many.sh <module> <list> <out>` | a whole address list in **one** Ghidra invocation across all 24 cores — 32,857 functions in 500 s, versus 13 h projected for the batched path |
| `decomp-all.sh` / `decomp-sweep.sh` / `decomp-watch.sh` | every function of a module into a per-function tree, resumable, with a heartbeat that distinguishes stalled from finished |
| `emu-module.py find` | where does 13.0.5 do the thing 2.0.0 did at this hardcoded address |

## "What is this number, or this name?"

| Tool | Question |
|---|---|
| `hash40.sh <hash \| name>` | both directions, plus `--len N` to enumerate every known name of a given length — the length is in hash40's top byte, so a guess is confirmed by hash rather than argued for |
| `luaconst --hashdict` | hashes of every identifier-like string in `main`'s `.rodata` and `.text` — 47,856, and the only source with C++ member names like `owner_object_id_` |
| `luaconst --order` | the order in which `main` registers its 40,338 constants |
| `emu-module.py consts` | **every lua constant's name and value for 13.0.5**, by emulating the registration itself |
| `vendor/refs/param-labels/ParamLabels.csv` | the first stop for any unknown param hash |

## "Is the port faithful?"

`check.sh` runs the whole offline harness (about four minutes) and is what `deploy.sh` gates on.
The individual gates, and what each would catch:

| Gate | Catches |
|---|---|
| `check-bones.sh` | a hitbox anchored to a bone Pikachu does not have |
| `check-hitboxes.sh` | a hitbox that does not land where the source's does, per frame, with recorded per-move ceilings |
| `check-acmd-values.sh` | a non-geometric macro argument that differs from the source's |
| `check-callparity.sh` | an engine call the source makes that the port does not — **a list of questions, not of bugs** |
| `check-nativecalls.sh` | the same, across all ported statuses, as a complete reading list. Exits 0 on purpose |
| `check-srcparams.sh` | a hand-written special using a value that is not the source's, or failing to declare it unmodelled |
| `check-status.sh` | another fighter's namespaced constants in hand-written code |
| `check-statusgraph.sh` | a transition into a status that is not fully registered |
| `check-statuslines.sh` | an Init, Exec, or Exit line the source has and the port does not |
| `check-scripts.sh` | an ACMD the shipped `motion_list.bin` names but nothing registers |
| `check-keepflags.sh` | a status flag the hand-written code reads that `deforeign` stripped from the ACMD that sets it |
| `check-victim.sh` | code putting another fighter into a status that dereferences an article we do not have |
| `check-lists.sh` | the three hand-maintained port lists disagreeing |
| `check-animsize.sh` | an animation set over the memory budget |
| `check-name.sh` | the CSS rename not being real UTF-16LE, or missing a label |
| `cached.sh` | runs a gate only if something it could read has changed since it last passed |

`inventory.sh` reports, per moveset slot, what is actually implemented, checked against the
shipped files rather than against intent.

## "What happened on the console?"

| Tool | Question |
|---|---|
| `watch.sh` | live telemetry off the console during a playtest, with constants resolved |
| `analyze.sh` | everything the console knows about the last session, in one command — move summaries first, then markers, then crash reports. Compares the log's build stamp against HEAD, because a stale plugin has burned a whole session |
| `pull-crash.sh` | Atmosphère crash reports and the Skyline log, newest first |
| `ftpq.sh` | an ad-hoc FTP query with the deploy credentials, retrying empty answers — sys-ftpd returns them |
| `trace-all.sh` | every dropped-in move's per-frame hitbox error against the source, by forward kinematics |
| `hbsvg.sh` | each traced move as a side view, so hitboxes can be looked at rather than only measured |

## "Get me the game's data"

| Tool | Question |
|---|---|
| `arc-fetch.sh` / `arc-pull` | paths out of the running game's `data.arc`, skipping what is already local. 97 files/s with the block cache |
| `pull-nro.sh` | the `lua2cpp` fighter NROs out of the running game |
| `pull-exedump.sh` | `main`'s own `.text`, `.rodata`, `.data`, and symbol table, out of the process |
| `fetch-status-scripts.sh` | all 95 fighters' decompiled status scripts from upstream |
| `fetch-refs.sh` | param labels, dumped motion lists, and the Rust readers |
| `emu-module.py ptrs` | the 67,651 function entry points no `bl` reaches, out of the relocated `.data` |

## "Generate the port"

Codegen, in build order: `gen-scales.sh`, `gen-motion-params.sh`, `gen-name.sh`, `gen-moves.sh`,
`deforeign`, `add-bones.sh`, `anim-dropin.sh`, `motion-patch` + `synth`, `gen-rebase-anims.sh`,
`reach rebase`, `topscale`, `dropin` pre-flight, then `cargo skyline build --release`.
`gen-config.sh` derives `config.json` from what the mod tree actually contains — a rule that
immediately found a file declared new which vanilla Pikachu already has. `gen-params.sh`,
`gen-vlprc.sh`, `gen-paramprovider.sh`, `gen-statusnames.sh`, `gen-selftest.sh`, and
`gen-buildstamp.sh` produce the rest.

**Run `build.sh`, not the passes.** Order matters, several passes consume each other's output, and
the build gates on the exit code plus the `.nro`'s mtime rather than on grepping cargo's output.

## "Ship it"

`deploy.sh` — the harness, then the plugin and the whole mod folder, then verification. It holds a
lock keyed on a live pid (`/proc/<pid>`, because the pid is an MSYS pid), records its manifest at
upload time rather than at verification time, verifies with targeted `SIZE` queries rather than a
directory listing, and repairs with backoff. **Run it alone**, in the background, output to a file.
`RESTORE.md` is the revert procedure.
