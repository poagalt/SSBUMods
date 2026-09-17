# The visuals: what is grafted, what cannot be, and what the rope would cost

Written 2026-09-14 on the instruction to "add all the correct visuals ... behavior should be the
same as the current chrom sword implementation".

## The mechanism, and why it is not an article

`tools/merge-weapons.sh` merges each source fighter's weapon into Pikachu's body model as a named
mesh object, bound to a bone grafted under his own `HaveR`, and `src/sword/mod.rs` shows it for the
motions that need it. Technique from LilyLavender Modding Workshop episode 39.

This deliberately avoided `smashline::clone_weapon`, which had crashed inside the game's own
`fighter_initialize_object_data` — the Ptooie blocker in `handbook/open-questions.md`. A merged mesh
needs none of that machinery: it is part of Pikachu's model, it inherits his skeleton's animation
for free, and a visibility flag is its entire lifecycle. That is still the right answer for a
weapon that is a stick, and the four grafts below stay whatever the clone does.

`CLONE_ENABLED` is now `true` — see "The clone, and the three articles waiting behind it" at the
end of this file.

## Shipped

| mesh | source | bone | material | shown for |
|---|---|---|---|---|
| `chrom_sword` | `chrom/model/sword` | `Sword` | `pasted__met_chrom_001` | jab |
| `cloud_sword` | `cloud/model/fusionsword` | `fusionsword1` | `def_cloud_004` | back-air |
| `mythra_sword` | `elight/model/esword` | `Sword1` | `alp_light_002` | dash attack, three tilts |
| `palutena_staff` | `palutena/model/body` | `Stick` | `metal_palutena_002_a` | neutral-air, grab |

All four bones hang off `HaveR` in their own fighter's skeleton, which is why the technique
transferred from Chrom unchanged. Every value is read out of the source's own files — mesh and
material from `model.numdlb`, bone and parent from `model.nusktb`, bounding sphere from each
article's `model.numshexb` rather than a radius anybody guessed.

Only `chrom/sword` was in the local extraction; the other 35 files came off the running console
with `tools/arc-fetch.sh`, and so did the twelve textures.

**The textures ARE shipped now, and the alias that replaced them is why nothing loaded for a day.**
This said "textures are still not shipped: twelve aliases in `tools/gen-config.sh` point Pikachu's
paths at each source fighter's vanilla bytes", offered as the thing that kept the mod free of
redistributed texture data.

`share-to-added` cannot read the vanilla arc. It resolves only when a mod on the SD card provides a
real file at the source path, and when it cannot, **ARCropolis hangs the fight load** rather than
erroring — no crash report, filesystem work completed, loading animation still spinning, no fighter
object ever created. Chrom's three aliases worked for weeks purely because the test console carries
an "Alucard over Chrom" mod that replaces those exact three textures; Cloud's, Mythra's and
Palutena's pointed at nothing. Episode 39's recipe ships the textures, which is precisely why it
ships them.

The value is intact regardless: `pikachu-moveset/mod/fighter/pikachu/model/` is gitignored, so the
twelve `.nutexb` are a local artefact exactly like the merged model. `config.json` now declares 42
shipped paths and **zero** aliases, and `tools/check-weapons.sh` asserts resolvability rather than
a mechanism — a referenced texture must be a real file in the shipped model directory, and an
aliased one earns a warning naming this failure.

**It used to be one sword for four fighters.** `SWORD_MOTIONS` pointed seven motions at
`chrom_sword`, so Cloud's back-air, Mythra's dash attack and her tilts all swung Chrom's Falchion.

## What the graft cannot do

**It is no longer a ONE-bone graft, and that sentence was a bug, not just a limitation.** The
merge took the single bone its table named. Three of the four weapons are *rigid* — the mesh has no
`bone_influences` and single-binds to one bone, which is exact for a stick and is why Chrom's
Falchion worked for weeks. Mythra's `lightsword_VIS_O_OBJShape` is **skinned**, weighted to
`Sword1`, `Sword_lightC` and `Sword_lightD`, and only `Sword1` was grafted.

That model shipped with a mesh weighted to two bones that do not exist in the skeleton. Vanilla
Pikachu has *zero* influence bones absent from his own `nusktb`, so it is a clean invariant, and
breaking it **hung the training-mode load forever** — three boots, twenty-five minutes on the
first. Not a crash: no crash report, ARCropolis finished its filesystem work and went quiet, the
plugin installed all the way through `articles::install()`, the loading animation kept spinning,
and no fighter object was ever created. A vanilla Mario loading fine in the same session is what
finally localised it to our Pikachu's assets.

`merge-weapons.sh` now derives the graft set from the mesh: the union of the table's bone and every
bone the influences name, walked in source-skeleton order so a parent is appended before its
children, each reparented to its source parent's merged index when present and to the table's
anchor when not. 77 → 83 bones, with `Sword_lightC` and `Sword_lightD` under the grafted `Sword1`,
exactly their source hierarchy. `check-weapons.sh` gates on the invariant.

**What remains out of reach is animation, not bone count.**

    gaogaen/rope        RopeC RopeL1..3 RopeR1..3 Throw_TR    the cord bends
    gamewatch/parachute Parachutea1 a2 b1 b2                  the canopy opens
    kamui/dragonhand    Hand1..4 HandD HandU                  the hand morphs

Those shapes come from each article's own `model.nuanmb`, and nothing in this port plays an
article's animation. The graft can now carry all of their bones, but a grafted bone has no
animation track, so it sits at bind pose relative to its parent — which is why the weapons work
(their parent `HaveR` is Pikachu's hand and carries the swing) and why these would not. For
Corrin's up-air that is merely wrong, so it keeps no weapon rather than the wrong one. For the rope
it is worse than nothing: a straight bar between Pikachu and wherever the victim happens to be.

The answer for all three is a real article. `clone_weapon` is proven to INSTALL but not yet proven
to let the fighter load — see the last section of this file for what is and is not established.

## The rope is not a policy choice, and ADR 0004 is not what I said it was

Worth stating plainly because I got it wrong twice in conversation: **ADR 0004 is "never hand a
victim to engine code written for another fighter"**, not "no articles". There is no ADR excluding
articles. The rope's absence is the ADR's PREMISE, not its conclusion — and the ADR's own evidence
is the strongest argument for building one:

```
Data Abort, fault address 0
  L2CFighterCommon::status_SwingGaogaenThrown + 0xf8
  -> app::lua_bind::LinkModule__get_parent_pos_impl
```

`status_SwingGaogaenThrown` reads its linked parent's position with no null check, because for
Incineroar the parent is **always the rope article**. No rope, no link, null pointer.

So the victim side of the Alolan Whip — the outbound leg, the bounce, the return, and the
positioning that decides whether a lariat connects — is engine-common code this port cannot use,
for one reason: there is no rope. A real rope would hand all of it back to the game.

That is also the answer to "this problem may have its roots in the article issue". It does — and the
route to a real article is `clone_weapon`, which **installs cleanly**: nine boots logged
`clone_weapon returned index 0` with no fault. Whether a fighter then LOADS is still untested.

The mesh graft still cannot substitute, because the rope's
whole job is to be a moving link between two objects and a grafted bone has no animation track.
What the rope needs now is the clone plus the victim side, not a mechanism that does not exist.

## Not implemented, with the numbers for whoever finishes it

**Revenge's emissive glow.** `special_lw_revenge_emmisive_scale_min = 0.1` and `_max = 1` in
gaogaen's `vl.prc`, applied through `ModelModule::set_emmisive_scale` — the misspelling in the
binding matches the param name exactly, which is how the mechanism was identified. What is missing
is the MAPPING and the inactive value: nothing in `lua2cpp_gaogaen.nro` or its effect script calls
`set_emmisive_scale`, and main's decompiled corpus has only the `lua_bind` thunk, so the call site
is in the unexported Gaogaen code — the same blind spot as the Revenge HIT/TURN branch. Two param
values and no evidence for how they are interpolated is not enough to implement under ADR 0005, so
it is recorded instead of guessed.

The active-Revenge state is still visible: `aura_on`/`aura_tick` put a `sys_final_aura` on the four
shoulder and knee bones and pulse `FLASH(1.0, 0.45, 0.0, 0.35)` orange, which is Incineroar's own
structure with a `sys_` effect substituted under ADR 0002.

**Incineroar's red is a model variant, not a tint.** `revenge_off` calls
`VisibilityModule::set(boma, 0xc8e6a844f, 0x11a164aef7)` — a visibility group and state, i.e. a
swap to a differently-textured variant of his own body mesh. Pikachu has no such variant, so a
tint is the only available approximation and the orange pulse above is it. Both hashes are
unresolved (12 and 17 characters) and are not in any label list.

**Little Mac's f-smash armor is already implemented** and looks absent because the word never
appears: it is `damage!(MA_MSC_DAMAGE_DAMAGE_NO_REACTION, DAMAGE_NO_REACTION_MODE_ALWAYS, 0)` on
script frame 8, back to `_MODE_NORMAL` on 14, copied verbatim and held by
`tools/check-acmd-values.sh`. What is NOT implemented is a cue when the armor actually absorbs a
hit. That needs a detector — damage rising while the no-reaction mode is on — rather than a
transcription, because the source has no such script either; whatever vanilla shows comes from the
engine's own armor path.

**Also found, and since fixed:** the native Revenge boost is applied through
`AttackModule::set_power_mul_5th` and `set_reaction_mul_3rd`, both of which `revenge_off` resets to
1.0 at `[boma+0xa0]` vtable 0x270 and 0x2b8. This port used neither. See the finish-camera section
below, which is where that audit ended up.

## The blue zoomed screen is the game's finish camera

> "the visual that happens for mac super armor and incin revenge lariat is a zoomed in blue
> screen, like the red kill screen but blue"

**What it is not**, so this is not searched a third time:

| Mechanism | What it actually is |
|---|---|
| `EffectModule::req_screen` | the Final Smash background. All 597 calls in the dumped scripts request a `bg_*_final` name and nothing else. |
| `FILL_SCREEN_MODEL_COLOR` / `CANCEL_FILL_SCREEN` | the Final Smash desaturate. All 160 calls pass `EFFECT_SCREEN_PRIO_FINAL` and a near-grey colour. |
| `FighterCutInManager` + `FighterUtil::request_critical_hit_cut_in_force` | the Kazuya / Joker cinematic cut-ins, keyed on a `CutInType` and a `CutInData`. |
| a `sys_*` armor effect | does not exist. Nothing matching "armor" is in either hash list or the 47,858-name hash40 table. |

**What it is.** The engine's *finish camera*: `sv_animcmd::CHECK_FINISH_CAMERA` (0x22af6c0),
`CHECK_FINISH_CAMERA_IF_NOT_HP_MODE` (0x22af920), `AttackModule::set_no_finish_camera`, and eleven
`FINISH_CAMERA` work ids — including `FIGHTER_INSTANCE_WORK_ID_FLAG_FINISH_CAMERA_DOLLY_STAGE`,
which pairs with `FighterUtil::request_critical_hit_cut_in_dolly_stage` (0x69b800). A dolly zoom on
the stage. That `set_no_finish_camera` exists at all says the engine offers the camera BY DEFAULT
and a fighter opts out, which is why Little Mac's f-smash script contains no camera call and still
gets the effect in vanilla.

**And the macro is entirely common-engine state**, which is what makes it portable at all.
`CHECK_FINISH_CAMERA` decompiles (`work/ghidra/main-22af6c.annotated.c`) to three calls on
`[boma+0x50]` and nothing else:

```
WorkModule vtable[0x110]  on_flag(0x2000009F)     FLAG_REQUEST_CHECK_FINISH_CAMERA_THROW
WorkModule vtable[0x60]   set_float(arg1, 0x42)   the macro's first argument
WorkModule vtable[0x60]   set_float(arg2, 0x43)   the macro's second
```

`0x42` and `0x43` sit just below `FIGHTER_INSTANCE_WORK_ID_FLOAT_FINISH_CAMERA_THROW_RAY_LENGTH`
(`0x44`), so all three are common instance ids — no fighter kind, no namespaced constant, nothing
this port has to translate. `_IF_NOT_HP_MODE` adds one check of the stamina-mode byte at
`0x52c51b2` and returns early.

That also settles what Incineroar's `critical_frame` / `critical_zoom_rate` / `critical_slow_rate`
/ `critical_zoom_in_frame` / `critical_camera_offset_x`,`_y` / `critical_1_on_1` params are *not*:
they are not this path. `tools/check-srcparams.sh` lists them as unmodelled in both
`param_special_s` and `param_special_lw`, and that is correct — they are read by HIS native code
keyed on HIS fighter kind, which never runs here. `src/params/table.rs` serves them anyway because
it is generated from the whole object, which is harmless.

**And half of it was missing here.** Both of Incineroar's side-B finishers ask for the camera:

```
gaogaen/SpecialSLariat.txt:11     macros::CHECK_FINISH_CAMERA(agent, 0, 0);
gaogaen/SpecialSShoulder.txt:16   macros::CHECK_FINISH_CAMERA_IF_NOT_HP_MODE(agent, 0, 0);
```

The lariat's survived into `specials_acmd/mod.rs`. The shoulder's did not: smash-script exports
`CHECK_FINISH_CAMERA` and not the stamina-mode variant, and `tools/reach/src/bin/deforeign.rs`
deletes calls to macros the crate cannot resolve rather than failing the build. The two lines AFTER
it survived, because `FighterCutInManager::set_throw_finish_zoom_rate` and
`set_throw_finish_offset` are plain `lua_bind` calls — so the port was setting that camera's zoom
rate to 1.5 and its offset to the origin and then never asking for the camera.

Closed three ways: `src/acmd_ext/mod.rs` supplies the missing wrapper (skyline-smash binds the
`sv_animcmd` entry point; only smash-script's four-line thunk was absent), `gen-moves.sh` emits
`use crate::acmd_ext as macros;` so a verbatim script reaches it, and `acmdparity.rs` now lists the
call so a future deletion is a failed check rather than a silent one. `build.sh` reads the
allowlist from both crates' sources, so it still cannot drift.

**The Revenge boost now uses Incineroar's own slots.** `revenge_off` resets
`AttackModule::set_power_mul_5th` and `set_reaction_mul_3rd` to 1.0 (`[boma+0xa0]` vtable 0x270 and
0x2b8, `work/ghidra/main-ab86f.annotated.c`). This port was writing `set_power_up` and
`set_reaction_mul` — the *generic* multiplier slots that rage, items and handicap also write, so
whichever ran last won. All four call sites in `speciallw` now write slot 5 and slot 3. That closes
the parity gap recorded above; whether it also restores the zoom is a playtest question.

## The clone, and the three articles waiting behind it

`src/articles/mod.rs` holds a two-step experiment. **STEP 2 HAS NEVER ACTUALLY BEEN RUN, and this
section said it had passed.** The correction matters because the whole Ptooie transcription was
about to be built on it.

What is confirmed is that the call *installs*. Nine boots logged

    [rat] articles: clone_weapon returned index 0 -- article id should be 0x6

with no crash and no fault, so `clone_weapon` itself is sound and the article index is 0x6 as
predicted. That is step 2's *first half*.

Step 2 asks a different question -- "does Pikachu load?" -- and the answer is **unknown**:

| boots | clone | fighter object alive | SELFTEST lines |
|---|---|---|---|
| `listen-boot`, `listen3`, `playtest-130602` (x3), `playtest-134418`, `listen2` b1, `playtest-132809` b1 | **on** | **0 of 9** | 0 |
| `listen2` b2, `playtest-132809` b2 | off | 2 of 2 | 4 each |
| `work/pt2.log` .. `work/pt40.log` | off | 40 of 40 | 4 each |

0-for-9 reads like a verdict, and it is not one, because **every clone-on boot on record predates
the fix for a bug that was hanging every load regardless.** The clone-on logs are stamped
2026-09-14 13:27-14:34 local; `share-to-added cannot read the vanilla arc` (b2469ae) landed at
17:52 the same day. During that window a load either hung or did not, for reasons that had nothing
to do with articles -- so those nine boots carry no information about the clone, in either
direction. `clone_weapon returned index 0` was read as the whole of step 2 passing; it is the half
that happens before the fighter is built.

The cost of the error was nearly a 14-status transcription resting on an untested premise. Ptooie,
the rope, the parachute and the dragon hand are **not yet known to be unblocked** -- one boot with
`CLONE_ENABLED = true` on a card with real textures settles it, and that is the cheapest
outstanding experiment in the project.

If Pikachu loads with the spikeball appended, all four constants exist and the remaining work is
three rows in `tools/gen-article-assets.sh` and three `clone_weapon` calls:

| article | weapon kind | donor's own id | assets in the arc |
|---|---|---|---|
| Ptooie's spiked ball | `WEAPON_KIND_PACKUN_SPIKEBALL` | `0x3` (packun) | 14 model, 10 motion |
| Incineroar's ropes | `WEAPON_KIND_GAOGAEN_ROPE` **and** `_ROPE2` | `0x0`, `0x1` | 14 model, 6 motion |
| G&W's parachute | `WEAPON_KIND_GAMEWATCH_PARACHUTE` | — | 16 model (incl. `model.nuanmb`), 6 motion |
| Corrin's dragon hand | `WEAPON_KIND_KAMUI_DRAGONHAND` | — | 19 model (incl. `model.nuanmb`), 26 motion |

The parachute and the dragon hand each ship their own `model.nuanmb`, which is exactly what a
one-bone graft could not reproduce and what a real article plays for free — so the clone route
answers the objection in "What a one-bone graft cannot do" directly rather than working around it.

**The rope is still the hard one, and not for asset reasons.** ADR 0004's crash evidence is that
`status_SwingGaogaenThrown` faults at address 0 because Incineroar's linked parent is *always* the
rope; a real rope hands the whole victim side back to engine code that assumes it. Two articles,
not one, and the victim's status reads through `LinkModule::get_parent_id`. Cloning it is necessary
but not sufficient.

**Every clone is c00-only right now.** A cloned article resolves under the owner's costume slot and
vanilla has no `fighter/pikachu/<article>` at any slot, so a non-c00 Pikachu has nothing to fall
back to. Recorded in `gen-article-assets.sh`; the fix is a loop over `c00..c07`, deferred so the
clone experiment stays one variable.
