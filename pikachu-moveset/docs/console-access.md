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

**Every attachment point from the George technique transfers.** Caveat: this mod restyles the
geometry, so its `numshb`/`numdlb` are not vanilla — but a restyle does not touch the skeleton,
so the `nusktb` is very likely vanilla bytes. Worth diffing against a real dump when we have one.

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
