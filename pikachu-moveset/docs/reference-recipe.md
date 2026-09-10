# The George recipe

Teardown of "George" v3.0 (LilyLambda / TCNick3, GameBanana 399292), the reference for building a
moveset mod without the CSK Collection. Everything here is evidence from the shipped files.

## How it avoids CSK

Two mechanisms, and only two:

1. **Every borrowed animation is re-exported onto the host fighter's own skeleton and written into
   the host's own motion folder.** Nothing ever references another fighter's motion directory.
2. **Every visual effect comes from the always-resident `sys_*` archive.** 22 distinct `sys_*`
   effects; a regex sweep for any fighter-prefixed effect name returns zero hits. Borrowing a
   source fighter's effects is exactly what would force a CSK dependency.

## Animations

56 `.nuanmb` per costume slot, byte-identical across `c00`–`c07`. Two cases:

- **Host already has that motion** → reuse the vanilla filename, it simply overrides. Declared in
  `config.json`'s `share-to-added` so `c01`–`c07` resolve to `c00`.
- **Host has no such motion** (18 files here) → invent a filename following the game's own
  category convention, then declare it in `new-dir-files` under the `fighter/<host>/cNN`
  dir-info node so the ARC has an entry to load.

The `c00`/`d02`/`e01`/`j01` filename prefixes are **Smash Ultimate's own category convention**
(`c0*` normals, `d0*` specials, `e0*` grab/throw, `j01` taunts), identical for every fighter —
they do not encode the source fighter.

`config.json` uses exactly two keys: `share-to-added` (40 sources → 280 targets) and
`new-dir-files` (8 costume keys × 21 paths). No `share-to-vanilla`, no `new-dir-infos`.

## motion_list.yml — where FAF and intangibility actually live

A full 746-entry YAML round-trip of the host's `motion_list.bin`, identical in all eight slots.
Each entry binds name → `game_script` → animation file → `[expression_, sound_, effect_]` triple,
and carries:

- `cancel_frame` — **this is FAF**
- `xlu_start` / `xlu_end` — **this is the intangibility window**
- `blend_frames` — forced to `0` on essentially every replaced entry, which stops the host rig's
  rest pose bleeding into a borrowed animation
- `flags` (turn / loop / move / fix_trans / …)

There are **no transition-term fields**; transitions are handled in the plugin via
`WorkModule::enable_transition_term_group` and `sub_transition_group_check_air_*`.

## Move scripts

One smashline `Agent` for the host fighter, installed **agent-wide** — the mod uses the
non-`_costume` installers, so the moveset applies to every costume slot. It resolves exactly four
symbols at runtime via `nn::ro::LookupSymbol`:

```
smashline_install_acmd_script      smashline_install_line_callback
smashline_install_status_script    smashline_install_state_callback
```

Roughly 23 `game_acmd` and 29 `effect_acmd` scripts. Frame data is written **in code, not in
params**: hitboxes via `sv_animcmd::ATTACK` / `ATTACK_ABS` / `ATK_HIT_ABS`, grab boxes via
`CATCH`, timing via `frame` / `wait` / `FT_MOTION_RATE`, movement via `sv_kinetic_energy::*`.

Notably absent: `smashline_clone_weapon` and `smashline_add_param_object` (so no new articles),
`smashline_get_original_acmd` (every script is a full replacement, never a chain), and any
`sound_*` or `expression_*` script.

## Appendages — the technique, and its limit

George bolts geometry onto the host **without adding a bone and without adding a named mesh
object**: two extra *sub-meshes* (sub-index 2 and 3) appended under an existing mesh-object name,
~1660 verts each, with a new material added to `model.numatb`. Because they are new sub-indices
of an existing name, the `numshexb` visibility-group table needs no edit. `model.nusktb` is
deliberately **not shipped** — the vanilla skeleton is untouched. George single-binds to five
existing bones: `HaveL`, `HaveR`, `HandR`, `ShoulderL`, `ShoulderR`.

**The limit, and why it exists:** the plugin's import table contains **zero `ModelModule`
entries**. No `set_scale`, no `set_mesh_visibility`, no joint scaling — George's appendage is
permanently attached and always visible. The cause is the technique itself: **reusing an existing
mesh-object name means there is no distinct name for `set_mesh_visibility` to target.** Anything
that needs to toggle must use a *distinct* mesh-object name instead, as LilyLavender's Modding
Workshop episode 39 does. See `sword-options.md`.

Runtime scaling still has no precedent here; George does no runtime model manipulation at all.

`model.nuhlpb` is shipped as an 88-byte empty stub, stripping all helper-bone constraints so
borrowed animations aren't fought by the rig. **We should not copy this** — Tier 1 drives
Pikachu's own animations, which need Pikachu's helper bones intact.

## Params

`fighter/common/param/fighter_param.prcx` is a **sparse `paracobn` patch**: a
`fighter_param_table` list where unmodified rows are the scalar placeholder `dummy` and only the
modded row carries overrides. Confirmed key names, which are exactly the attribute set we need:

| Key | George's value |
|---|---|
| `weight` | 135.0 (Bowser) |
| `walk_speed_max` | 1.47 |
| `dash_speed` (initial dash) | 2.42 |
| `run_speed_max` | 2.31 |
| `run_accel_mul` | 0.10362 |
| `ground_brake` | 0.116 |
| `air_speed_x_stable` (air speed) | 1.26 |
| `air_speed_y_stable` (fall speed) | 1.7 |
| **`air_accel_y` (gravity)** | 0.12 |
| `air_brake_y` | 0.2 |
| `air_accel_x_mul` / `air_accel_x_add` | 0.085 / 0.01 |
| `jump_y` (full hop) | 44.5 |
| `mini_jump_y` (short hop) | 21.35 |
| `jump_aerial_y` (double jump) | 44.2 |
| `jump_initial_y` | 24.475 |
| `jump_speed_x_mul` | 0.95 |

Fast-fall speed is an unresolved hash holding 2.72 (ZSS's value). Authoring these by name needs
`ParamLabels.csv`, which is **not on this machine** — it is public at
`ultimate-research/param-labels`.

## Packaging

A plain ARCropolis mod directory: `info.toml` + `config.json` + loose `fighter/` and `sound/`
trees + a root `plugin.nro` that ARCropolis auto-loads. No `ui/` overrides, no `effect/` folder.
Dependencies: ARCropolis, the smashline plugin, and whatever consumes `motion_list.yml`.
