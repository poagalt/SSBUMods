# Environment audit

Audited 2026-09-09 on this machine (`C:\Users\poaga`, single drive, 334 GB free).

## Present and usable

- **`libsmashline_plugin.nro`** in `Downloads` — 303,104 B, upstream `HDR-Development/smashline`,
  a late-2025-or-newer build. Exports 21 `smashline_*` symbols; George's `plugin.nro` imports
  only four of them, so this runtime is a superset of what the reference mod needs. Supports
  **hot-reloading a development plugin** from
  `sd:/atmosphere/contents/01006A800016E000/romfs/smashline/development.nro`.
- **ARCropolis 4.0.9** (`Downloads/release/.../libarcropolis.nro`). Hard-pinned to Smash
  **13.0.5**.
- **Atmosphère 1.11.2** + **hekate 6.5.3** SD bundles. Console-side only.
- **mingw-w64 GCC 16.1.0** at `~/w64devkit/bin` and Git's `mingw64` — viable Rust host toolchain.
- George's mod: 448 `.nuanmb`, `motion_list.yml` (466 KB per costume slot), `vl.prc`,
  `fighter_param.prcx`, and a `config.json` using exactly two ARCropolis keys —
  `share-to-added` (40 entries) and `new-dir-files` (8 entries). That pair is the whole
  no-CSK mechanism.

## Absent

| Missing | Blocks |
|---|---|
| Rust, rustup, cargo, cargo-skyline, `aarch64-nintendo-switch-freestanding` | compiling `plugin.nro` |
| Blender + Smash addons, `ssbh_data_json` | authoring animations (moot — script-first) |
| **`data.arc` / any romfs dump / the game itself** | **source frame data for 17 fighters** |
| Ryujinx / Yuzu / Sudachi, and any Smash ROM | testing in-game locally |
| devkitPro | nothing — cargo-skyline ships its own toolchain |

## The one that matters

> **CLOSED as of 2026-09-15 — see the re-audit below. This section is kept as the record of why
> `extracted/` exists, not as a current statement of the environment.**

There is **no source of source-fighter frame data on this machine.** The parity contract needs
hitbox parameters for Chrom, Mythra, Terry, Falco, R.O.B., Little Mac, Fox, Game & Watch,
Palutena, Kazuya, Cloud, Corrin, K. Rool, Piranha Plant, Incineroar, DK and Mewtwo. All of it
lives in `data.arc`. Every other gap is an install; this one is not.

---

## Re-audited 2026-09-15: three of the five gaps are closed

Checked because the console had been updated to 23.0.0, which Atmosphère does not support, and the
question was what could still be done without it. The answer turned out to be almost everything,
and this file was the reason it looked otherwise.

| Was "absent" | Now | Evidence |
|---|---|---|
| Rust, rustup, cargo, cargo-skyline | **present** | rustc 1.98.1, cargo 1.98.1, cargo-skyline 3.5.0, toolchain `skyline-v3`. `cargo skyline build --release` exits 0 and writes `librat_of_doom.nro`, 245,760 B |
| `ssbh_data_json` | **present** | `vendor/ssbh_lib/ssbh_data_json.exe`; `check.sh` section 3 round-trips every shipped model through it |
| **`data.arc` / a romfs dump** | **present** | `extracted/data.arc`, 629,145,600 B, and `extracted/fighter/` with 98 fighter directories |
| Blender + Smash addons | still absent | moot, as originally noted -- script-first |
| Ryujinx / Yuzu / Sudachi, and any Smash ROM | **still absent** | no binary on PATH, no emulator data directory. This is the only gap that still blocks anything |

**All seventeen source fighters the parity contract names are present**, by internal name, 450–692
files each: chrom, elight (Mythra), dolly (Terry), falco, robot, littlemac, fox, gamewatch,
palutena, demon (Kazuya), cloud, kamui (Corrin), krool, packun, gaogaen, donkey, mewtwo.

### The console, as of 2026-09-17

It was accidentally updated to **HOS 23.0.0**, which no released Atmosphère supports (1.11.2 tops
out at 22.5.0) and no released hekate supports either (6.5.3, same ceiling). It now runs
**Atmosphère 1.12.0 built from the upstream `23_support` branch**, and reports
`23.0.0 | AMS 1.12.0 | S`. The whole mod stack works on it: Skyline, ARCropolis 4.0.9,
`librat_of_doom`, arc-network and training_modpack all load, and `deploy.sh --verify-only`
passes 98/98.

That branch is **not buildable from public sources as shipped** — its pgl rework calls libnx
symbols that exist in neither released libnx 4.12.0 nor libnx master. Seven are renames onto
identical IPC command ids and one (`pglEnableApplicationCrashReport2`, cmd 31) is stubbed because
CMIF and TIPC pack arguments in different orders, so its wire layout cannot be read off the C++
declaration. Build environment and the patched sources: WSL2 + devkitPro (devkitA64/devkitARM
16.1.0); artifacts in `C:\Users\poaga\Atmosphere-23.0.0-build\` and
`C:\Users\poaga\hekate-23.0.0-build\`.

**A patched hekate exists but is NOT deployed.** The card still boots by RCM-injecting
`fusee.bin` directly, which keeps hekate out of the boot path entirely. Deploying hekate is a
separate experiment; do not conflate a failure there with an Atmosphère problem.

**Check the internal name, not the display name.** Four of those seventeen come back MISS if you
look for `mythra`, `terry`, `kazuya` or `corrin`, which reads exactly like a missing dump rather
than like the wrong key. `CONTEXT.md` is the place that settles it.

So the honest current statement is narrower than this file's original one: **everything except
running the game is available locally.** Build, generate, and all 38 gates of `tools/check.sh`
need no console and no emulator. What is genuinely blocked is in-game behaviour -- the up-B
recovery and the grafted-bone hitboxes among them -- and no install on this machine fixes that.
