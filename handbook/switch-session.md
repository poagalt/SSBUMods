# The next Switch session

Written 2026-10-12 with no console available, so that the next sitting with one is a single,
ordered job rather than a discovery session. The tooling is `tools/switch-session.sh`; this page
is the procedure and the reasons.

## What is already done without one

Everything below was built offline from what this PC already holds:

| Built | What it gives | Page / tool |
|---|---|---|
| ELFs for `main` and **96** fighter NROs | each module with its own symbols, relocations, imports, and status functions named `<KIND>__<FUNC>` | `tools/rea-elf.py`, [rea.md](rea.md) |
| A kept, fully analysed Ghidra project for all of them | the hour-long analysis paid once, in WSL at `/srv/ssbu/ghidra` | `tools/code-index.sh` |
| The code index | every function, symbol, **code and data** reference and string, plus a decompile, in one SQLite file | `tools/code-index.py`, `work/code-index/` |
| The move index | per fighter: motions → ACMD (hitboxes, effects with colours, sounds, articles) → animations (frame counts) → statuses → params, and which game assets are or are not on this PC | `tools/move-index.py`, `work/move-index/sheets/<fighter>.md` |

## What only the console can do

| Missing | Why it cannot be done offline | What it unlocks |
|---|---|---|
| **`prod.keys`** | Derived inside the console's security processor, so only software running on the console can produce them; Lockpick_RCM is the usual tool, and nxdumptool reads them from `sdmc:/switch/prod.keys` (traps.md). **The BIS keys on this PC** (`SwitchConsoleBackup-2026-09-15/*_BISKEYS.bin`) decrypt the NAND's partition filesystems, not the game's NCA containers, which need the master-key-derived keys in `prod.keys`. So the 28 GB `SwitchNAND-…/rawnand.bin.*` backup alone does not yield the game | everything in the next three rows, and reading this PC's own NAND/SD backups |
| **The full `data.arc`** (17,600,880,952 B) | Lives inside encrypted NCAs. `extracted/data.arc` is the first 3.6% of an abandoned network pull and cannot be opened | every vanilla effect archive (`.eff`, none local), every sound bank (none local), body models and textures for ~72 of 91 fighters, all c01–c07 animations, Sora's 387 c00 animations, stages, UI |
| **The ExeFS** | Same containers | the real `main` NSO (its initial `.data`, not a running game's), and `rtld`/`sdk`/`subsdk*`, the modules `main`'s 2,205 imports resolve into |
| `lua2cpp_item`, `lua2cpp_trail` | Inside `data.arc` | Sora's status scripts and the item module, the two of 98 NROs not on this PC |
| **The game running** | It is the game | playtests, telemetry, anything a trace records (`the-recording.md`); no analysis substitutes |

Where the base game is installed is not known: the SD card's `Nintendo/Contents` totals
7.09 GiB, less than `data.arc` alone, so it is on the console's own storage or on the cartridge.
nxdumptool reads it from whichever, so this does not change the plan.

## The session

**Before the console is on:** `bash tools/switch-session.sh check`. It lists what is ready on this
PC and what is not. Two things it cannot do, which need poagalt:

- A **Lockpick_RCM** payload that supports **HOS 23.0.0** (the console's firmware). It is not
  vendored here and should come from a source you trust; an older build will not know 23.0.0's
  master key.
- Binding the **libusbK** driver with Zadig to the console's nxdumptool USB device
  (VID `057E`, PID `3000`), once. It is a Windows driver change. `check` reports whether it is
  bound once the console is in nxdumptool's USB mode.

**1. Keys.** Power off, enter RCM, inject Lockpick_RCM the same way as fusee
(`TegraRcmSmash.exe -w <payload>`; see the boot procedure in the project memory). It writes
`sdmc:/switch/prod.keys`. Copy that file to this PC **outside the repository**, for example
`%USERPROFILE%\.switch\prod.keys`, and never commit or share it: the keys are specific to this
console.

**2. Dump.** Reboot into Atmosphere (`fusee.bin`), open nxdumptool (already on the card), and dump
Smash's **ExeFS** and **RomFS** with **the installed update applied**, so the files match what the
game actually runs (13.0.x) and what every memory dump here came from. On this PC:
`bash tools/switch-session.sh dump` receives it over USB (`tools/nxdt-host`, already set up and
smoke-tested). One sequential 16.4 GiB stream, instead of the abandoned network pull's
~170 minutes and two game crashes.

**3. After the dump:** `bash tools/switch-session.sh post-dump work/switch-session/dump`. It:
- checks that `data.arc` is exactly 17,600,880,952 bytes, and stops if it is not;
- extracts the **whole** archive into `extracted/` with the existing `arc-bulk`, which already
  takes a local `data.arc` and is ~100× faster than the network;
- wraps the real `main` NSO as `work/rea/main-nso.elf` (`rea-elf.py nso`, LZ4 and SHA-256 checked);
- rebuilds every NRO ELF, now including `item` and `trail`, and the move index.

Then `bash tools/code-index.sh all` in the background (~1 h for `main`).

**4. Only if there are no keys:** pull from the **running** game instead, in priority order. Lists
come from `bash tools/switch-session.sh lists` (computed from the move index's asset table) and go
one at a time through `bash tools/switch-session.sh pull <list>`, at the measured ~2.5 files/s:

| List (`work/switch-session/`) | Paths | At ~2.5 files/s | What it gives |
|---|---|---|---|
| `pull-01-nros.txt` | 2 | seconds | `lua2cpp_item` and `lua2cpp_trail` (Sora) |
| `pull-02-effect-archives.txt` | 96 | ~1 min by count, longer by bytes | every fighter's vanilla `ef_<f>.eff`: the effects and colours the move index can only name today |
| `pull-03-sora-c00-motion.txt` | 375 | ~3 min | Sora's c00 animations, the one fighter without them |
| `pull-04-c00-models.txt` | 14,219 | ~1.5 h | c00 models, materials and textures for every fighter body and article |
| `pull-05-effect-other.txt` | 769 | ~5 min | the rest of `effect/fighter/` |
| `pull-06-sound.txt` | 7,350 | ~50 min | sound banks |
| `pull-07-other-costumes.txt` | 356,924 | ~40 h, so dump route only | c01–c07 |

The rate is the measured per-file one (BACKLOG: latency-bound, ~110 KB/s overall); effect archives
and sound banks are large files, so `arc-bulk <addr> extracted --dry-run` with a list's filters
gives the real byte count before committing to one.

**5. Hardware-only work.** Nothing in steps 1–4 competes with it except step 4, which needs the
game open. From the 2026-10-12 offline pass (BACKLOG.md, last section), in this order, **one
variable per boot**:

1. **The current build as it stands.** The up-B now drives the cloned parachute: spawned hidden on
   the rise, opened on the drift, folded on the close (`specials/specialhi*`). The observable is a
   parachute that appears and goes away exactly when Game & Watch's does, and no parachute left
   behind after landing, a ledge grab, or being hit out of the drift.
2. **The whip's ledge box.** Flip `WHIP_CLIFF_HANGDATA`, `bash tools/gen-vlprc.sh`, build, deploy.
   First observable: Pikachu loads at all. The last shipped vl.prc did not. Then whip toward a
   ledge beside Incineroar doing the same.
3. **The rope's vtable neutraliser**, only once 1 and 2 are settled. Set the `rope` row
   `enabled: true`, regenerate assets and config, build, load. Pass is the log line
   `NEUTRALISED rope … for entry N` and no crash; the rope is not used by the whip yet, so nothing
   visible changes. The spikeball is the same experiment for Ptooie, and goes after.

## After it

The move index's sheets show, per fighter, how many of the game's model, effect, motion and
sound files are on this PC; after step 3 every count becomes "all". The code index can then be
rebuilt from the real NSO, with the SDK modules beside it.
