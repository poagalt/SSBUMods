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

`ultimate/mods/Ken Sugimori style Pikachu` was renamed to `.Ken Sugimori style Pikachu`.
ARCropolis skips dot-prefixed folders. **All its files are intact** — remove the leading dot to
re-enable it.

It conflicted with this mod on six files:
`fighter/pikachu/model/body/c00/model.{adjb,numatb,numdlb,numshb,numshexb,nusktb}`. Only one mod
can own those, and ours carries the Falchion mesh. Re-enabling it means the restyle wins and the
sword silently stops rendering — `set_mesh_visibility` on a mesh that is not in the loaded model
is a no-op, so nothing breaks, the blade just will not appear.

The stale `ultimate/arcropolis/conflicts.json` was deleted so ARCropolis regenerates it cleanly.
