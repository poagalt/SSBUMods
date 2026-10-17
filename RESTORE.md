# Restoring the console

Everything this project added to the console, and how to undo it. All paths are on the SD card.

## Added by this project

| Path | What | Remove to undo |
|---|---|---|
| `atmosphere/contents/01006A800016E000/romfs/skyline/plugins/librat_of_doom.nro` | The moveset plugin | yes |
| `atmosphere/contents/01006A800016E000/romfs/skyline/plugins/libarc_network.nro` | Serves `data.arc` over TCP 43022 while the game runs | yes |
| `switch/nxdumptool.nro` | RomFS dumper, homebrew menu entry | yes |
| `atmosphere/contents/420000000000000E/` | `sys-ftpd` background FTP sysmodule | delete the folder |
| `config/sys-ftpd/config.ini` | Its credentials and port | delete with the above |

Nothing else was modified. No pre-existing file was overwritten, and no game data was touched.

## Full revert

Delete the five paths above and reboot. The console returns to exactly its prior state:
ARCropolis, smashline, nro_hook, param_hook, Training Modpack, George, and 43 mods, all
untouched.

## Partial reverts

- **Moveset misbehaving:** delete `librat_of_doom.nro` only.
- **Suspecting a plugin conflict:** delete `libarc_network.nro` too — it is only needed while
  extracting assets.
- **Don't want an always-on FTP service:** delete `atmosphere/contents/420000000000000E/` and
  `config/sys-ftpd/`, and go back to launching `ftpd-classic` by hand.

## Known-good builds

`releases/v0.1/librat_of_doom.nro` — 22 moves, no attributes, no motion_list patch.
Tagged `v0.1-moves`.

## Known-good builds, continued

`releases/v0.2/` — adds attributes, the motion_list patch (FAF, intangibility, Fox dodges) and
Game & Watch's up-B.

**To roll back to v0.1** (moves only, vanilla recovery and vanilla attributes):

1. Upload `releases/v0.1/librat_of_doom.nro` over the installed one.
2. Delete the mod folder `ultimate/mods/Rat of Doom/`.

The up-B is the highest-risk component — it replaces the recovery and could not be tested here.
If the character cannot recover, or the game hangs on selecting Pikachu, do the two steps above.

## v0.3 — sword and remaining motion work

`releases/v0.3/` adds Chrom's Falchion, Little Mac's angled f-smashes, and the sword toggle.

**This is the first version that modifies Pikachu's model files** (`numshb`, `nusktb`, `numdlb`,
`numatb`, `numshexb`). A malformed model file makes the character fail to load. All five were
validated by round-tripping through `ssbh_data_json`, but they are untested in game.

**If Pikachu crashes the game or fails to appear:** delete
`ultimate/mods/Rat of Doom/fighter/pikachu/model/` on the console. That drops back to vanilla
geometry while keeping the moveset, attributes and motion patch. The blade simply will not
render; `set_mesh_visibility` on a mesh that does not exist is a no-op.

## Note on what is in git

The repo holds **tools and source**, not game-derived binaries. Model files, patched
`motion_list.bin` and the `extracted/` tree are gitignored: they are derivatives of Nintendo
assets and all of them are reproducible.

```bash
bash tools/gen-moves.sh        # move scripts from vanilla ACMD
bash tools/gen-params.sh       # attribute prcxml
bash tools/merge-sword.sh      # sword into Pikachu's model
tools/motion-patch/target/release/motion-patch.exe extracted <out.bin>
tools/motion-patch/target/release/synth.exe        extracted <out.bin> <out.bin>
cd pikachu-moveset/plugin && cargo skyline build --release
```

Rebuilding requires `extracted/`, which comes from the console via `tools/arc-pull` while
`arc-network` is loaded and the game is running.

## Disabled mod

`Ken Sugimori style Pikachu` was **moved out of the scan path** to
`ultimate/mods-disabled/Ken Sugimori style Pikachu`. **All its files are intact** — move the
folder back into `ultimate/mods/` to re-enable it.

**A dot prefix does NOT disable a mod in ARCropolis 4.0.9.** That was tried first and the very
next `conflicts.json` still listed `sd:/ultimate/mods/.Ken Sugimori style Pikachu` in all six
entries. The dot-prefix convention comes from older loaders; 4.x uses workspaces/presets. Moving
the folder outside `ultimate/mods/` is what actually works.

**And renaming does NOT disable a SKYLINE PLUGIN either — Skyline loads EVERY file in
`romfs/skyline/plugins/`, whatever its extension.** Found on 2026-09-17 while trying to isolate a
crash: `librat_of_doom.nro` was renamed to `librat_of_doom.nro.bak` on the assumption that only
`*.nro` is loaded. The next boot ran the moveset anyway — `rat-telemetry.log` grew from 204 bytes
to 71,450 with a full SELFTEST and live MIRROR lines — so the "plugin disabled" control run was
never a control at all, and would have been read as "the platform crashes even without the mod"
had it crashed.

The same rule as the mods directory applies: **move the file OUT of `plugins/`**, do not rename it
in place. And a `.bak` left in that directory is not inert, it is a second loaded plugin.

It conflicted with this mod on six files:
`fighter/pikachu/model/body/c00/model.{adjb,numatb,numdlb,numshb,numshexb,nusktb}`. Only one mod
can own those, and ours carries the Falchion mesh. Re-enabling it means the restyle wins and the
sword silently stops rendering — `set_mesh_visibility` on a mesh that is not in the loaded model
is a no-op, so nothing breaks, the blade just will not appear.

The stale `ultimate/arcropolis/conflicts.json` was deleted so ARCropolis regenerates it cleanly.

## v0.9–v0.11 — the source fighters' own animations

This is the first version that **replaces Pikachu's animation files**. Eleven `.nuanmb` files are
now shipped in `fighter/pikachu/motion/body/c00/`, each being the source fighter's own animation
for that move, scale-compensated (see ADR 0003 and `tools/anim-dropin.sh`). SSBU names an
animation the same on every fighter, so these are plain same-path replacements.

| file | source | move |
|---|---|---|
| `c00attack11.nuanmb`    | Chrom       | jab |
| `c00attackdash.nuanmb`  | Mythra      | dash attack |
| `c01attacks3s.nuanmb`   | Mythra      | f-tilt |
| `c02attackhi3.nuanmb`   | Falco       | u-tilt |
| `c03attacks4s.nuanmb`   | Little Mac  | f-smash |
| `c04attackhi4.nuanmb`   | Fox         | u-smash |
| `c05attackairn.nuanmb`  | Palutena    | n-air |
| `c05attackairf.nuanmb`  | Kazuya      | f-air |
| `c05attackairb.nuanmb`  | Cloud       | b-air |
| `c05attackairhi.nuanmb` | Corrin      | u-air |
| `c05attackairlw.nuanmb` | K. Rool     | d-air |

**If a move animates wrongly or the game hangs entering a match:** delete
`ultimate/mods/Rat of Doom/fighter/pikachu/motion/body/c00/*.nuanmb`, keeping `motion_list.bin`.
Pikachu falls back to his own animations. The moves still work and keep their frame data, damage
and knockback, but limb-anchored hitboxes go back to landing in the wrong place — the defect these
files exist to fix.

**Pikachu deforms during these moves, and that is expected.** The animation carries the source's
own bone translations, so he takes on their proportions and his limbs extend to reach the hitboxes.

`model.nusktb` gained two bones in v0.9, `Sword1` and `Stick`, grafted under `HaveR` by
`tools/add-bones.sh`. Chrom's jab and Palutena's n-air anchor hitboxes to them. Rolling back the
model directory alone will therefore make those two moves' hitboxes fail to spawn.

**To roll back to v0.8** (re-anchored hitboxes, Pikachu's own animations):

```bash
git checkout v0.8-pre-nuanmb
bash tools/build.sh && bash tools/deploy.sh
```

Note that v0.8 and earlier contain the defects fixed in v0.9–v0.11: hitboxes frozen at one frame
instead of sweeping, 14 capsule far ends unable to reach the plane fighters stand on, and the
source fighters' own work-flag ids being set on Pikachu (K. Rool's belly-armor flag is Pikachu's
`FLAG_PAUSE`).
