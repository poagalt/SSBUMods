# Engine APIs and upstream sources

## The single biggest find: George's source is public

**`https://github.com/LilyLavender/ssbuGeorgeMoveset`** — the reference mod's actual Rust source.
Everything in `reference-recipe.md` was reverse-engineered from the shipped `plugin.nro`; the
source makes that unnecessary going forward. It confirms the binary analysis: no
`set_mesh_visibility`, no model work at all, only `Hash40::new("haver")`/`("havel")` as hitbox and
`EFFECT_FOLLOW` / `AFTER_IMAGE4_ON_arg29` anchors, plus
`ArticleModule::generate_article(*FIGHTER_DIDDY_GENERATE_ARTICLE_GUN, …)`. **George is a reference
for *driving* a hand-anchored weapon, not for adding one.**

## Upstream sources worth having

| Repo | What it gives us |
|---|---|
| `LilyLavender/ssbuGeorgeMoveset` | The reference mod's full source |
| `LilyLavender/ssbu-mod-workshop` — **episode 39** | "Adding A Sword": working `config.json` + `mod.rs` putting Chrom's sword on Little Mac |
| `LilyLavender/ssbu-mod-workshop` — **episode 46** | `ArticleCloningGuide.md`, by PhazoGanon & LilyLambda, plus a worked cloned-article example |
| **`WuBoytH/SSBU-Dumped-Scripts`** | **Vanilla ACMD dumps for every fighter, weapon article and item.** A second source of truth alongside rubendal — and a better one for armor and intangibility windows, since it is the actual script rather than a hitbox table. |
| `HDR-Development/HewDraw-Remix` | Real-world usage of the constraint and article APIs |
| `HDR-Development/smashline` | `clone_weapon` and the rest of the Smashline 2 surface |
| `Demonslayerx8/Smash-Ultimate-Hash-Names` | Full ARC path list |
| `ultimate-research/param-labels` | `ParamLabels.csv`, and `articles/Labels.txt` for free article indices |
| `skyline_smash` crate source (`src/cpp.rs`, `src/lua_const.rs`) | Every `lua_bind` signature — docs.rs renders nothing for this crate |

## Attaching a prop — `set_joint_parent` does not exist

`ModelModule`'s complete function list is: `set_rotation_order`, `rotation_order`, `scale`,
**`set_scale`**, `scale_z`, `set_scale_z`, `set_temporary_scale_z`, `joint_global_position`,
`joint_global_position_with_offset`, `joint_global_offset_from_top`,
`top_joint_global_position_from_joint`, `joint_global_rotation`, `joint_global_axis`,
`set_joint_srt`, **`set_joint_scale`**, `set_joint_rotate`, `set_joint_translate`,
`clear_joint_srt`, `joint_rotate`, `set_visibility`, `is_visible`, **`set_mesh_visibility`**,
`set_alpha`, `set_color_rgb`, `set_emmisive_scale`, `set_render_offset_position`,
`set_depth_offset`, `set_depth_stencil`, `virtual_joint_tra`, `enable_gold_eye`,
`disable_gold_eye`. **No parenting call.**

So the two mechanisms are distinct:

- **Merged mesh** (our sword plan) → single-bind in the model data, toggle with
  `ModelModule::set_mesh_visibility`.
- **Separate article** → `LinkModule::set_model_constraint_pos_ort(article_boma, *LINK_NO_ARTICLE,
  Hash40::new("have"), Hash40::new("haver"), CONSTRAINT_FLAG_POSITION | _ORIENTATION, false)`, or
  the convenience wrapper `ArticleModule::have(boma, article_id, Hash40::new("haver"), …)`.

The convention is that an article's own skeleton carries a bone named `have` (sometimes
`attach`), and that bone is mated to the fighter's `HaveR` / `HaveL`.

**Runtime scaling: the API exists.** `ModelModule::set_scale` and `set_joint_scale` are confirmed
present. We still have no worked example of a mod using them, but this is no longer speculative.

## Single-binding a mesh to a bone

Authoritative, from the Blender plugin wiki: *"Vanilla smash uses this very frequently on
fighters for weapon meshes or for expression meshes."* The recipe in `ssbh_data` terms:

- `mesh_object.parent_bone_name = "HaveR"`
- `bone_influences = []` (empty for meshes without vertex skinning)
- pre-transform `positions` by `inv(skel.calculate_world_transform(HaveR))`, and `normals` /
  `tangents` / `binormals` likewise — vertices are stored in the bone's local space

Vanilla's own in-hand / on-back / sheathed swords are visibility *groups*
(`VisibilityModule::set_int64(boma, hash40("sword"), hash40("sword_normal"))`), each variant
single-bound to a different bone.

## Naming and bones

- `.nusktb` strings are CamelCase (`HaveR`); scripts and hashes are lowercase
  (`Hash40::new("haver")`). Verified: `crc32("have")` matches the literal used in vanilla item
  scripts; `crc32("Have")` does not.
- `TransN` / `RotN` / `LHaveN` are **Smash 4** names, not Ultimate's.
- Fighter bone indices: 0 `top`, 1 `trans`, 2 `rot`, 3 `hip`, 12 `waist`, 15/21
  `shoulderl`/`shoulderr`, 17/23 `handl`/`handr`, 19 `head`, **24/25 `havel`/`haver`**, 26 `throw`.
- Articles live under the fighter, not `weapon/`:
  `fighter/<fighter>/model/<article>/c##/model.nusktb`.

## Correction on Chrom

Chrom has **no scripted articles** — his sword is a mesh on his body model. A
`fighter/chrom/model/sword/` path exists in the ARC list, so the geometry is obtainable to merge,
but there is no article to clone. That keeps option 1 (merge into Pikachu's body) as the Chrom
route and option 2 (Ganondorf) as the article route.
