# Console access

Target: modded Switch 1, Smash **13.0.5**, `<console-ip>`.

## FTP — working, read *and* write

`ftpd-classic` v3.2.1-2 on port **5000**, anonymous, full SD root. Verified by uploading a file,
reading it back and deleting it.

```bash
curl -l ftp://<console-ip>:5000/                     # list
curl -T local.file ftp://<console-ip>:5000/path/     # write
curl -o out.file ftp://<console-ip>:5000/path/file   # read
```

**What this makes autonomous:** deploying builds to `/ultimate/mods/`, dropping a
`development.nro` for smashline hot-reload, installing Skyline plugins, reading ARCropolis logs,
and reading any installed mod's files.

**What it can never reach:** `data.arc`. The game archive lives in the title's RomFS, not on the
SD card. No amount of FTP gets at it.

## What is on the card

| | |
|---|---|
| Skyline plugins | `libarcropolis`, `libnro_hook`, `libparam_hook`, `libtraining_modpack`, `libsmashline_plugin`, and `plugin.nro` — **which is George (`george_moveset`, 106,496 B), already installed and running** |
| Mods | 43 in `/ultimate/mods/` |
| Homebrew | NX-Shell, ftpd-classic, Goldleaf, JKSV, daybreak, haze. **No nxdumptool. No arc-network.** |
| ARCropolis | `cache/`, `config/` (two workspace files), `logs/` (empty) |

Note our plugin must **not** be named `plugin.nro` — George already occupies that filename in the
plugins folder. Both would load fine otherwise; George registers the `diddy` agent and ours
registers `pikachu`, so there is no agent conflict.

## Already banked: Pikachu's skeleton

The installed "Ken Sugimori style Pikachu" mod ships a complete body model set. Pulled
`model.nusktb` (21,800 B) to `reference-assets/pikachu_c00_model.nusktb`. Confirmed bones:

```
Trans  Rot  Hip  Waist  Bust  Neck  Head  EarL1  EarR1  Have  Hat  Face
ClavicleC/L/R  ShoulderL/R  ArmL/R  HandL/R  HaveL  HaveR
Tail1-4  LegC  LegL/R  KneeL/R  FootL/R  ToeL/R  Throw  Mouth
```

**Every attachment point from the George technique transfers.** Superseded below: the assumption
that a restyle leaves the skeleton alone turned out to be **false** — see "Two results worth
keeping". Use the extracted vanilla `nusktb`, not this one.

## Getting at `data.arc`

| Option | How | Cost |
|---|---|---|
| **`arc-network` Skyline plugin** (recommended) | I install the `.nro` over FTP; ArcExplorer on this PC then pulls individual files out of the **running game's** archive over the network | Minutes per file. Only manual step: launching the game. .NET 6 runtime is already installed for ArcExplorer. |
| **nxdumptool** | I install the `.nro` over FTP; it dumps the title's RomFS | `data.arc` is a single ~16 GB file, so there is nothing selective about it — but dumping it once makes this machine permanently self-sufficient. 334 GB free. |
| ARCropolis dump feature | — | Does not exist. |

## Live logging

Skyline logs over TCP, independent of FTP: `cargo skyline listen --ip <console-ip>`. Given there
is no local test path, this is the difference between reading a panic and guessing at one.
`cargo skyline set-ip` / `install` also exist but assume FTP on port 21, not 5000 — needs checking.

## Extraction checklist

From the console, once `arc-network` is up:

- [ ] `fighter/pikachu/model/body/c00/` — `model.numdlb`, `numshb`, `numatb`, `nusktb`,
      `numshexb`, `nuhlpb`, `adjb`, `xmb`, `nusrcmdlb` (**vanilla** — the real forcing item)
- [ ] `fighter/pikachu/motion/body/c00/motion_list.bin` — codegen owns this file; patch a real
      round-trip rather than a reconstruction
- [ ] `fighter/chrom/model/sword/c00/` — full model set plus `met_chrom_001_*` textures
- [ ] `fighter/pikachu/param/vl.prc` — vanilla baseline
- [ ] `fighter/common/param/fighter_param.prc` — vanilla baseline for the sparse `.prcx` patch

From GitHub, no console needed:

- [ ] `ultimate-research/param-labels` → `ParamLabels.csv` and `articles/Labels.txt`
- [ ] `WuBoytH/SSBU-Dumped-Scripts` → vanilla ACMD for the source fighters
- [ ] `LilyLavender/ssbu-mod-workshop` → episode 39 (sword), episode 46 (article cloning)
- [ ] `LilyLavender/ssbuGeorgeMoveset` → the reference mod's real source

## Extraction — done

`arc-network` loaded and served on TCP 43022. Its source is a thin wrapper: it opens
`rom:/data.arc` and exposes it through `network_reader`, a seek/read RPC. **`smash-arc` already
speaks that protocol** via `ArcFile::open_over_network`, so no protocol work was needed — see
`tools/arc-pull`, a ~30-line Rust client built against `smash-arc 0.6` with the `rust-zstd`
backend (the pure-Rust zstd feature; the crate refuses to build without a backend selected).

**23 of 23 files extracted, zero failures**, into `extracted/` (gitignored — these are Nintendo
assets and must not be redistributed; ship `share-to-vanilla` references instead).

| File | Bytes |
|---|---|
| `fighter/pikachu/model/body/c00/model.numshb` | 1,033,384 |
| `…/model.adjb` | 168,824 |
| `…/model.nusktb` | 21,800 |
| `…/model.numatb` | 10,444 |
| `…/model.numshexb` | 4,192 |
| `…/model.numdlb` / `.nusrcmdlb` | 3,218 each |
| `…/model.nuhlpb` | 2,124 |
| `…/model.xmb` | 1,312 |
| `fighter/pikachu/motion/body/c00/motion_list.bin` | 43,428 |
| `fighter/pikachu/param/vl.prc` | 6,385 |
| `fighter/common/param/fighter_param.prc` | 176,604 |
| `fighter/chrom/model/sword/c00/` (9 files + 3 textures) | 1,078,294 total |

### Two results worth keeping

**The Ken Sugimori skeleton assumption was wrong.** Its `model.nusktb` is the same *length* as
vanilla (21,800 B) but a different hash — `5324967…` vs vanilla `4cc9491…`. Restyle mods do
touch the skeleton, presumably transform tweaks with the bone list unchanged. Use the extracted
vanilla file, not the mod's.

**Chrom's sword is exactly as predicted.** `model.nusktb` is 664 bytes with two bones,
`HaveR` → `Sword`, and `model.numdlb` holds a single mesh object, `fal_RShape`. That matches the
research prediction bone-for-bone and byte-for-byte, and `model.nuhlpb` is the familiar 88-byte
empty stub.

## Local sources

| Path | Size | Contents |
|---|---|---|
| `vendor/ssbuGeorgeMoveset` | 268 KB | The reference mod's real source — `src/{normals,aerials,smashes,specials}/` |
| `vendor/ssbu-mod-workshop/episode39` | — | `config.json` + `mod.rs`, the sword template |
| `vendor/param-labels` | 5.8 MB | `ParamLabels.csv`, `articles/Labels.txt` |
| `vendor/SSBU-Dumped-Scripts` | 309 MB | Vanilla ACMD under `luas/` |
| `vendor/ArcExplorer` | 41 MB | GUI fallback; unused, `tools/arc-pull` supersedes it |
