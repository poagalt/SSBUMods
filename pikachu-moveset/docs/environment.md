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

There is **no source of source-fighter frame data on this machine.** The parity contract needs
hitbox parameters for Chrom, Mythra, Terry, Falco, R.O.B., Little Mac, Fox, Game & Watch,
Palutena, Kazuya, Cloud, Corrin, K. Rool, Piranha Plant, Incineroar, DK and Mewtwo. All of it
lives in `data.arc`. Every other gap is an install; this one is not.
