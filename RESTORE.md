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
