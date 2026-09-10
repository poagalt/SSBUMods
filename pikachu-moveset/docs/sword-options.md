# Sword asset options

## Corrections to earlier assumptions

1. **Blender is not required.** `ssbh_data_json` (standalone Windows binary from
   `ultimate-research/ssbh_lib` releases) round-trips `.numshb` to JSON with plain float vertex
   arrays and `bone_influences` keyed by bone *name*. `ssbh_data_py` ships
   `examples/combine_modls_meshes.py`, which appends one model's mesh objects and modl entries
   into another. Merging a sword into a body model is a JSON edit, not mesh surgery.
2. **Show/hide has a published precedent.** LilyLavender's Modding Workshop **episode 39,
   "Adding A Sword"** puts Chrom's Falchion on Little Mac: mesh named `chrom_sword`, bound to
   `haver`, three textures via `new-dir-files`, toggled per-frame with
   `ModelModule::set_mesh_visibility`, and a trail via
   `AFTER_IMAGE4_ON_arg29(..., "tex_item_killsword1", "tex_item_killsword2", 4, "haver", ...)`.
   That is exactly the string sequence in George's `plugin.nro` — **George is built on this
   recipe**, which also explains its otherwise-unexplained `sword1` literal.
3. **A game dump is obtainable without ripping the game.** ArcExplorer plus the `arc-network`
   Skyline plugin browses and extracts from `data.arc` over the network by IP. Atmosphère and
   Skyline are already running on the target console.

## Sword trails and ADR 0002

`effect/system/item/trail/` holds `tex_item_killsword1-4`, `tex_item_beamsword1` and
`tex_item_deathscythe1-3` — in the always-resident `effect/system/` tree. Every *fighter* sword
trail (`tex_chrom_sword1`, `tex_cloud_sword1`, `tex_master_sword1`, …) is fighter-prefixed and so
forbidden by ADR 0002. **The Killing Edge trail is the only sword trail this mod may legally use,
and it ships no files at all.**

## Item swords are not viable as models

Every item is a self-contained `item/<name>/` model directory with no costume variants —
structurally ideal. Killing Edge (`item/killsword/`) is the cleanest single mesh in the game for
this purpose: 1 mesh object, 1 material, 2 textures. **But item-archive residency is
undocumented, there is no known force-load technique, and smashline has zero item API.** Treat
item models as not reliably resident. The trail textures above are a separate, resident tree and
are fine.

## Weapon as article vs. welded into the body

| Separate `fighter/<name>/model/<weapon>/` | Welded into `model/body/` |
|---|---|
| Byleth (`sword`, `sword2`, `swordflare`, `spear`, `axe`, `bow`), Ike (`sword`, `ragnell`), Cloud (`fusionsword`), **Chrom (`sword`)**, Roy (`roysword`), Ganondorf (`sword`), Pyra/Mythra (`esword`), Steve, Sheik (`knife`), Simon/Richter (`axe`) | Link, Young Link, Toon Link, Shulk (Monado), Sephiroth, Meta Knight, Marth, Lucina, Robin, Joker, Bayonetta, Hero, Mii Swordfighter, Sora |

For Chrom, Roy, Cloud and Ike the weapon exists in **both** places. Chrom's body model carries
`fal_L`, `fal_R`, `fal_e_VIS_O_OBJShape` and `SayaM_VIS_O_OBJShape` — so **Nintendo itself uses
the mesh-in-host-body approach for Fire Emblem swords.** The George technique is the vanilla
technique.

## Ranked options

| # | Option | Needs a dump? | Cost |
|---|---|---|---|
| 1 | **Chrom's Falchion baked into Pikachu's body mesh** (episode 39 method) | Yes — Chrom's sword *and* Pikachu's vanilla body model | 1 mesh, 1 material, 3 textures, ~150 KB. Published step-by-step with working `mod.rs`. Supports per-move show/hide. No article plumbing. |
| 2 | **Ganondorf's sword as a cloned article** via `smashline::clone_weapon` | Yes — `fighter/ganon/model/sword/` | Zero mesh editing. Pikachu confirmed cloneable; `GANON_SWORD` confirmed cloneable; new article ID `0x7`. Ganondorf is the only fighter whose sword textures aren't duplicated in his body. **Breaks if Ganondorf is in the same match** — model folders collide. |
| 3 | **Byleth's `spear` or `swordflare`** | **No — already on this machine** | Vanilla Nintendo geometry, 2-bone rig (`HaveR`→`Spear`), 1 mesh object, `swordflare` is 300 KB. Proves the pipeline today with no console session. Not a Falchion, and the textures are the modder's recolor — must be swapped before release. |
| 4 | Killing Edge item model | Yes | Cleanest mesh in the game, but residency is unproven and it buys nothing over #1. |
| 5 | Ike's `ragnell` / Sheik's `knife` | Yes | Smallest possible footprint — model dir only, zero motion or script files. Reference, not shipping. |

## Gotchas that George does not cover

- **Pikachu has a `model.adjb` (168 KB); Diddy does not.** Only Chrom, Cloud and Pikachu have
  body adjacency buffers in the whole roster. Any `numshb` edit must keep it consistent.
  `ssbh_data_json` handles `.adjb`.
- **George binds to five points**, not two: `HaveL`, `HaveR`, `HandR`, `ShoulderL`, `ShoulderR`.
- **George cannot toggle visibility** because he reused an existing mesh-object name with new
  sub-indices. Episode 39 uses a **distinct** name so `set_mesh_visibility` can target it. Use
  episode 39's approach.
- Runtime hashes are lowercase (`Hash40::new("haver")`); `.nusktb` strings are `HaveR`.
- Pikachu's body dir has no metal texture, so new textures must be declared in `new-dir-files`.
- Pikachu's skeleton (~151 bones) has `Have`, `HaveL`, `HaveR`, `HandL`, `HandR`, `ShoulderL`,
  `ShoulderR` — every George attachment point transfers.

## Attribution

Cleanest is first-party geometry extracted yourself, with unmodified `.nutexb` textures left
vanilla via ARCropolis `share-to-vanilla` so the game serves its own bytes. The merged
`model.numshb` is a derivative and must be shipped — unavoidable and universal for moveset mods.
**Do not ship the Byleth weapon textures currently on disk**: those are a third-party recolor,
not Nintendo's.
