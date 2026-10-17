# REA: asking the game's code questions interactively

[REA](https://github.com/morluto/rea) ("Reverse Engineer Anything") is an MCP server that puts
Ghidra and Hopper behind agent tools: decompile one function, list its callers and callees, find
the code that uses a string, trace a value through a function, name things as you go. Set up
2026-10-12. This page is how it fits *this* project; the generic workflow is the
`reverse-engineer-anything` skill in `%USERPROFILE%\.claude\skills`.

## When to reach for it, and when not

The corpus this project already has answers most questions faster than any live session:

| Question | Fastest answer |
|---|---|
| "Does anything in `main` do X?" — a text search over every function | `grep` over `work/ghidra/main-all-100k.c` (100,508 bodies, already decompiled) |
| One function as readable C, with `lua_const` reads and hash40s resolved | `tools/decomp.sh <module> <addr>` |
| A lua2cpp status without the L2CValue marshalling noise | `disasm --trace` |
| **Who calls this, with what, and what does the callee do with it** — following references several hops | **REA** |
| **Which code uses this string / this data address / this GOT slot** | **REA** (`search_strings`, then `xrefs`) |
| **Where does this argument come from** inside a function | **REA** (`trace_native_values`, function dossiers) |
| A second decompiler's opinion on a function Ghidra renders badly | **REA** with `provider_id: "hopper"` |

REA does not replace transcription from the decompiled source (ADR 0006); it makes reading it
faster. Its output is Ghidra or Hopper pseudocode, not the game's source, and every result says so.

**For `main`, use the code index first** ([lookup.md](lookup.md)): the same full Ghidra analysis
REA would run, done once and kept, so `tools/code-index.py callers|refs-to|strings|fn` answers in
under a second instead of after an hour. Reach for a REA session when a question needs something
the index does not keep -- data-flow tracing, function dossiers, or Hopper's second opinion -- and
on the fighter NROs, where a session costs ~30 s.

## What is installed, and where

REA runs **inside WSL** (the `Ubuntu` distro), not on Windows. Its Windows Ghidra mode admits only
x86/x64 PE files and Hopper has no Windows build, so on Windows it cannot open AArch64 Switch code
at all. Claude Code stays on Windows and starts the server as
`wsl.exe -d Ubuntu --exec /usr/local/bin/rea-mcp` (user-scope MCP registration `rea`).

| Piece | Where |
|---|---|
| rea-agents 6.4.0 (npm) | WSL, `/opt/node-v24.21.0-linux-x64/lib/node_modules/rea-agents`; `rea` on PATH |
| Ghidra 12.1.4 + OpenJDK 21 | WSL, `/opt/ghidra_12.1.4_PUBLIC`, `/usr/lib/jvm/java-21-openjdk-amd64` |
| Hopper 6.4.2 Linux **demo** | WSL, `/opt/hopper` — REA drives it on a private Xvfb display |
| The server's environment | WSL, `/usr/local/bin/rea-mcp-env` (heap, deadlines, default provider) |
| The skill | Windows, `%USERPROFILE%\.claude\skills\reverse-engineer-anything`, copied from the installed package so it matches the server |
| The targets | `work/rea/*.elf`, built by `tools/rea-elf.py` (below) |

All of it is reproducible: `bash tools/rea-wsl.sh setup` installs or repairs everything,
`update` moves to the newest REA release and re-copies the skill, `check` runs both engines'
readiness checks. REA ships several releases a week; after `update`, restart Claude sessions so
they pick up the new server.

The project's own Ghidra (`vendor/tools/ghidra_12.1.3_PUBLIC`, Windows, used by `decomp*.sh`) is
separate and untouched.

## The targets: `tools/rea-elf.py`

REA 6.4.0 opens a native target only when Ghidra recognises its header. `main` is a headerless
memory dump and the fighters are NROs, so **neither opens as-is**: REA's raw-binary mode exists
only on its unreleased main branch, and maps one blob with no names even there. So each module is
wrapped as an ordinary AArch64 ELF:

```bash
python tools/rea-elf.py main                  # work/rea/main.elf        ~2 s
python tools/rea-elf.py nro pikachu gaogaen    # work/rea/lua2cpp_<m>.elf
python tools/rea-elf.py all                    # main and every NRO in vendor/nro
```

Everything in the ELF comes from **the module's own tables**: MOD0 for `.dynamic` and the `.bss`
bounds, `.dynsym` for every export, `.rela.dyn`/`.rela.plt` applied exactly as rtld would, and each
PLT stub named after the import its `JUMP_SLOT` binds. The project's symbol files are a
cross-check, and the build prints it: **4,918 of 4,918** of `rat-main-symbols.txt`'s rows and
**489 of 489** of `gaogaen-nro-symbols.txt`'s are reproduced from the modules' own tables. Each ELF
has a `.json` manifest beside it with input digests and every count.

| In the ELF | From |
|---|---|
| Segments at the project's addresses: `main` at **0x0**, NROs at **0x7100000000** | as `symbols-merged.txt`, `*-nro-symbols.txt`, `decomp-many.sh` |
| Exports, demangled by Ghidra | `.dynsym` |
| Imports as a `.extern` block, one named 8-byte slot each; GOT and vtable slots point there | `.rela` against undefined symbols — how Ghidra's own loader shows them |
| PLT stubs named after their import | `.rela.plt` + the stub's own `adrp`/`ldr` |
| Status functions: `<STATUS_KIND>__<FUNC>`, e.g. `FIGHTER_STATUS_KIND_SPECIAL_S__STATUS_MAIN` | `disasm --statusmap` from `create_agent_fighter_status_script_<m>`, cached as `work/rea/<m>-statusmap.txt` |
| `main`'s lua-binding method names (3,980, e.g. `GetJointPos`) | `vendor/main/symbols-merged.txt` |
| Function seeds `sub_<addr>`: `bl` targets and RELATIVE-relocation targets in `.text` | `work/acquire/main-entrypoints.txt`, `work/decomp/<m>.todo.all`, `.rela.dyn` |

Seeds are link-time facts only. Pointers that exist only in the live process are **not** seeded:
a seed inside a real function splits it, which is the 667-spurious-boundaries trap in
[data-index.md](data-index.md). Ghidra's own names (`FUN_…`, `PTR_…`, `DAT_…`) are what its
analysis found by flow.

`main`'s `.data` was copied out of a running game, so it holds runtime values, not initial ones.
Relocation targets are rewritten from `.rela` (exact); the 7 other words that pointed into that
run's image are rebased and counted in the manifest. `.rodata` holds no absolute pointers and is
verbatim; `.bss` is empty, as the loader leaves it.

Rebuild the ELFs whenever `vendor/main/` or an NRO is re-pulled. REA identifies a target by its
SHA-256, so a rebuilt ELF is a new target and old Evidence will not replay against it.

## Using it from Claude

```json
{"name": "open_binary", "arguments": {"path": "/mnt/c/Users/poaga/SSBUMods/work/rea/lua2cpp_pikachu.elf"}}
```

- **Paths are Linux paths.** `C:\Users\poaga\SSBUMods\work\rea\main.elf` is
  `/mnt/c/Users/poaga/SSBUMods/work/rea/main.elf`. A Windows path fails (see Traps).
- Ghidra is the default provider (`REA_ANALYSIS_PROVIDER=ghidra`); pass `provider_id: "hopper"`
  for Hopper. Both can open these ELFs and REA will not guess between them.
- `open_binary` only selects the target. **The first real query pays for import and full
  auto-analysis**, and every later query in that session is instant:

  | Target | First query (Ghidra, cold) | Then |
  |---|---|---|
  | `lua2cpp_gaogaen.elf` (1.7 MB) | ~30 s | ~0.1 s per query |
  | `lua2cpp_common.elf` (8.5 MB, 14,288 procedures) | ~2–2.5 min | |
  | `main.elf` (87 MB, 103,596 symbols) | **3,915 s (65 min)**, measured 2026-10-12 through the CLI | not measured yet |

  The 65 minutes is almost all Ghidra's default auto-analysis. It ran at ~1 core for the first
  ~20 minutes, then in parallel (`StackVariableAnalyzer` on ~14 cores), and peaked at ~7.5 GB RSS
  under the 8 GB heap with no full GCs.
- **Why the registration carries `"timeout": 10800000`.** A cold import is *silent*: rea-agents
  6.4.0 sends a "started" progress event and nothing more until the answer. Claude Code aborts an
  MCP call that is silent for its idle window, 30 minutes by default. Measured with the window
  forced down to 45 s: *"tool binary_overview sent no response or progress for 60s; aborting"*
  on a cold `common`. A per-server `timeout` replaces that window for this server only, and with
  it the same call completed, both from `--mcp-config` and from the user-scope entry. 3 h is the
  same allowance `REA_GHIDRA_STARTUP_TIMEOUT_MS` gives Ghidra. `rea-wsl.sh setup` writes it, and
  repairs an entry without it. Calls past 2 minutes are also moved into a background task, which
  is expected.
- **Each session is a fresh, throwaway Ghidra project.** Closing the binary (or restarting
  Claude) throws away the analysis, and the next open pays the whole cost again. For `main`, open
  it once and keep asking within that session. Renames and comments made through REA
  (`set_addresses_names`, `annotate_native_function`) live only in that session too: write
  conclusions into the repo, not into the database.
- One session holds one target at a time; `open_binary` with another path switches it.

## Traps

| Trap | What it looks like |
|---|---|
| Opening `rat-main-text.bin` or a raw `.nro` | `target_unavailable: unsupported binary format`. Open the `work/rea/*.elf` instead |
| A Windows path | to Linux, `C:\…` is a relative path: `open_binary` refuses relative paths, and the CLI resolves it against its cwd and reports ENOENT |
| A registration without `"timeout"` | a cold import longer than Claude Code's idle window (30 min) is aborted as *"sent no response or progress"*. `main` takes 65 min, so it always is. Re-run `rea-wsl.sh setup`, and never re-add `rea` with plain `claude mcp add` |
| **Hopper truncates hash40 switch cases to 32 bits** | Incineroar's agent dispatch on `0x135015dde2` reads as `case 1343610338` (its low 32 bits) in Hopper; Ghidra keeps the 40-bit compare. Do not read hash40s out of Hopper's switch labels |
| `search_strings` hits in `.strtab:` | the ELF's own symbol-name table is mapped as a block too. Hits in `.rodata` (`0x…`) are the game's; `.strtab:0x…` ones are this wrapper's |
| Two names for the same function | a status function shared by several kinds carries one name per kind; Ghidra shows one as primary. Look the function up by address when a name is ambiguous |
| A PLT stub with no name | it calls the module's **own** export through the PLT (3,955 of them in `common`); Ghidra shows it as a thunk to the definition. Unnamed on purpose: the export's name on both would make the name ambiguous to REA |
| Hopper is the demo | REA selects demo mode each session, which is enough for analysis; nothing is saved, and nothing needs to be |

## Status, 2026-10-12 (paused mid-session)

**Verified working:**
- `rea-wsl.sh check`: Ghidra and Hopper both healthy, skill v37 matching REA 6.4.0, and the
  `rea` registration Connected.
- A fresh headless Claude session found `FIGHTER_STATUS_KIND_SPECIAL_S__STATUS_MAIN` by name in
  `lua2cpp_pikachu.elf`, decompiled it, and listed its named callees, using only the MCP tools.
- `main.elf` through the CLI: `generate_rope` at `0xabe800` comes back with its one 32-byte caller
  (`0x21ab100`) and named imports. What that decompile showed is now in
  `pikachu-moveset/docs/the-article-path.md`.

**Open, in order:**
1. **Make `main` affordable.** 65 minutes per fresh session is too slow. REA's main branch (not
   in 6.4.0) has `REA_GHIDRA_DISABLED_ANALYZERS`. A build of it sits in WSL at `/opt/rea-main`
   (commit `653067d`, run as `node /opt/rea-main/scripts/rea.mjs`), and has been checked to apply
   and record an override on `lua2cpp_pikachu.elf`. Next: run `analyzeHeadless` on `main.elf`
   once to get Ghidra's per-analyzer time summary (a run was started and stopped when the machine
   was closed; it never finished). Then disable only analyzers that do not feed
   references (keep the constant-reference and data-reference ones) and re-time. Then choose:
   wait for the next release and put the list in `/usr/local/bin/rea-mcp-env`, or point the
   launcher at `/opt/rea-main`. The released 6.4.0 is still what is registered.
2. ~~A cold `main` call through Claude Code dies at the idle timeout.~~ **Fixed 2026-10-12** with
   the per-server `timeout` (above), proven by forcing the idle window down to 45 s and running a
   cold `common` through the registered server. No global setting was changed.
3. Measure warm-query time on `main` (the "then" cell above). `work/rea/mcp_probe.py` is the stdio MCP client used to verify the server; it speaks to `wsl.exe … rea-mcp` exactly as Claude Code does.
