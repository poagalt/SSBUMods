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

## Facing left: two defects found by measurement, and what the axis of the blade says

Three weapons were wrong facing left -- Chrom's jab, both Mythra moves, Palutena's n-air -- while
Cloud's back-air and Corrin's up-air were correct in both directions. Ten rounds of reasoning from
structure produced candidates and two regressions. What follows is what measurement produced.

### First: the graft's frame is not the bug, and that is now algebra rather than a hunch

A rigid weapon mesh single-binds to a bone `B` one level under the weapon MODEL's own `HaveR`,
which is the frame the engine aligns with the fighter's hand. So natively

    v_world = v . world_w(B) . world_w(HaveR)^-1 . world_pika(HaveR)

and because `world_w(B) = local_w(B) . world_w(HaveR)`, the middle pair cancels, leaving exactly
`v . local_w(B) . world_pika(HaveR)`. That is precisely what grafting `B` under Pikachu's `HaveR`
with its own local transform and single-binding the mesh to it computes. Both source models have
that shape -- `chrom/model/sword` is `Root->HaveR->Sword`, `cloud/model/fusionsword` is
`Root->HaveR->fusionsword1` -- so **Chrom's and Cloud's grafts are algebraically identical to the
native attachment.** No frame error is possible for them, which closes a suspicion that cost two
playtests, and it agrees with what the earlier experiments already said: the pre-bake graft and the
bone-local bake are the same geometry, and the root-chain version was worse.

### Second: Chrom's jab was shipping 51 tracks for bones that do not exist

`anim-dropin.sh` had a fast path -- if the source fighter's `scale` param equals Pikachu's there is
no scale compensation to apply, so `cp` the animation across and skip the JSON round trip. Correct
when scaling was all the script did; silently wrong once the FOREIGN-TRACK TRIM was added, because
the trim has nothing to do with the ratio.

Chrom's `scale` is 0.9600 and Pikachu's is 0.9600. **He was the only source fighter in the moveset
sitting exactly on that fast path.** His jab therefore shipped with all 96 of his own Transform
tracks against an 88-bone skeleton, 51 of them naming bones Pikachu does not have -- `fingerl12`,
`fingerl13`, `fingerl20` and so on. An audit of all 74 shipped animations found exactly two with
foreign tracks: his, and `g02cliffattack` from Ganon, also 0.9600. Every animation that took the
slow path had zero.

It also skipped the shipped dump. `cp` never wrote `work/fk/shipped_<stem>.json`, which is the OURS
side `hbtrace` prefers -- so for the one animation carrying the defect, the harness was measuring a
file that was not what shipped, and could not have seen it.

Fixed by removing the fast path; the stamp cache above it already skips unchanged animations, so the
cost is about 0.7s on a genuine change. `tools/check-anim-tracks.sh` now asserts the OUTPUT across
all 74, because the build printing "0 trimmed of foreign tracks" was read for months as "nothing
needed trimming".

**The same hole existed a second time, in the cache itself.** The trim's output depends on the bone
list as much as on the source animation, and the stamp did not include it. Baking Mythra took the
skeleton from 88 bones to 79 and removed `Sword1` -- which CHROM's jab animates -- and all 74
animations were still reported "unchanged and skipped". The gate caught it because it reads the
shipped bytes; the cache is what should not have needed catching. `SELFSIG` now covers the bone
list, the mesh list and the realise pairs.

### Third: Mythra's sword was the one mesh whose construction was provably wrong

Hers is the only SKINNED graft, and skinning does not reduce to the native formula. It computes

    v_world = v . Wbind_pika(B)^-1 . W_pika(B)

substituting PIKACHU's bind pose for the weapon model's hand frame. Those are different matrices,
and what survives is the bone's DEVIATION from bind pose rather than its absolute pose -- so the
mesh renders near the model origin and drifts with the arm instead of sitting in the hand. Measured
on her blade tip: the native formula puts it at `(3.347, 2.014, 10.409)`, which is 10.86 units from
Pikachu's `HaveR` and so exactly the blade's own length, while the shipped skinning put it at
`(-0.019, 0.000, 10.874)` -- **3.951 units out at bind pose**, and worse in motion.

The bake is EXACT here rather than merely better, for two checked reasons. Every one of her skin
weights is exactly 1.0 (4 subindices over 9 influence bones, `Sword1` and `Sword_lightA..F2`), so
there is no blending to lose; and NO shipped Mythra animation drives `Sword1` or any light bone --
`c00attackdash` and `c01attacks3s` carry 43 Transform tracks each and not one names them. The whole
sword is rigid relative to `HaveR` for the entire move, so collapsing nine bones into one constant
matrix discards nothing. That is also why a single bake matrix serves vertices weighted to nine
different bones: at bind pose `inverse(world_esword(HaveR))` maps all of them into the hand frame
correctly, and bind pose is the only pose they are ever in.

Verified after the fact rather than assumed: the shipped tip now sits at `(3.347, 2.014, 10.409)`,
**0.0004 world units** from the native prediction, 10.874 from `HaveR`.

The earlier "with the bake, Chrom and Mythra still wrong left" did not test this. That bake was
GLOBAL, and it was measured while Chrom's jab was shipping those 51 foreign tracks. Both weapons
were confounded. The bake is now per-weapon and applies only where the unbaked form is provably
wrong; the four weapons that provably match native keep their grafts, since Cloud was CORRECT on
grafted `fusionsword1` and went wrong when baked onto `HaveR` with a matrix that is provably the
identity for him.

### The axis of the blade, which is the strongest unexplained signal left

Measured in each weapon's own bind frame, against the playtest verdicts for the state where Chrom
and Mythra were wrong facing left and Cloud and Corrin were right:

| weapon | bind | X extent | Y extent | Z extent | long axis | verdict |
| --- | --- | --- | --- | --- | --- | --- |
| chrom_sword | `Sword` | -1.40 .. 1.40 | -0.19 .. 0.18 | -1.83 .. 10.91 | **Z** | wrong left |
| mythra_sword | skinned | -1.62 .. 1.62 | -0.45 .. 0.45 | -2.28 .. 10.87 | **Z** | wrong left |
| cloud_sword | `fusionsword1` | -0.58 .. 0.58 | -3.80 .. 16.32 | -1.59 .. 1.59 | **Y** | correct |
| corrin_sword | `HaveR` | -0.38 .. 0.38 | -2.09 .. 13.28 | -1.37 .. 1.38 | **Y** | correct |
| palutena_staff | `HaveL` | -0.80 .. 0.85 | -10.82 .. 9.51 | -2.68 .. 2.72 | Y, centred | wrong left |

**Every one of the five is X-symmetric**, so a mirror in X is invisible on all of them. The two that
failed are the two extending along Z; the two that worked extend along Y and are near-symmetric in
both X and Z, so they are invariant under a 180-degree rotation about Y -- which is what
`PostureModule::rot_y_lr` is named for, and which negates X and Z and leaves Y.

That is the signature of a facing rotation being applied to, or omitted from, a weapon's frame while
the body gets the other treatment. It is a correlation over four weapons, not a mechanism, and it is
recorded here as the lead it is. Palutena is the one that does not fit, and she is also the only
weapon whose bind bone was replaced outright -- the vanilla `HaveL`, reparented under `HaveR` with
Stick's bind transform -- so she has a distinct mechanism and is expected to behave differently.

Worth noting for whoever picks this up: after the bake, Mythra's blade extends along local **+Y** in
`HaveR`'s frame, `(0.000, 10.874, 0.019)` -- the same axis as Cloud and Corrin. So the bake moves her
onto the axis of the two weapons that were never broken, which is a second and independent reason to
expect it to help, and it makes her the clean test of the axis correlation.

### The probe that answers it, and what it is missing

`telemetry::mirror_probe` has only ever logged POSITIONS, and a position cannot tell a 180-degree
turn from a mirror -- both send the hand to the same place, and only one turns the blade inside out.
`ModelModule::joint_global_rotation` is in the binary's lua_bind surface at `0x2057680` with a clean
`Vector3f` out-param, and had never been called. It now logs a `MIRRORROT` line per sample, raw and
with no `lr` applied, since dividing by the answer would erase it.

The probe was also reading a bone that does not exist. It logged `stick`, and `add-bones.sh` stopped
creating `Stick` when it began repurposing the vanilla `HaveL` -- so every Palutena reading in the
last round was a read of a name no skeleton contains, which is noise, and hers were the ones that
had "looked meaningful". It now reads `havel`. That is the measurement that decides where the
remaining bug can live: her staff is the only weapon bound to a bone Pikachu already owned, so if
`havel` tracks `handr` across both facings -- and `handr` is a fixed offset from `haver` by
construction, so it must -- then the skeleton is carrying the staff correctly and no further
skeleton work can fix any of these.

And `attack_air_b` was never sampled at all. Cloud's back-air is the only weapon the playtest has
ever called correct in BOTH facings, and a broken sample is only interpretable next to a working one
taken by the same instrument on the same frame budget. All six ported sword motions are sampled now.

### What George's mod does, since it works on both sides

`work/george` is his model directory, seven files, and the useful part is what is NOT there: **no
`model.nusktb`.** He does not touch the skeleton at all. His graft is entries 43 and 44 of the
numshb, appended as extra SUBINDICES of an existing `_VIS_` mesh name
(`diddy_Eye_Ottotto_VIS_O_OBJShape`, whose subindices 0 and 1 are the real eyes on `Head`), each
single-bound with `infl=0` -- one on `HaveR`, one on `HaveL`.

Two hands is not a facing mechanism, which was the first thing it looked like. The two copies are
1661 and 1656 vertices with different X and Z extents and an identical Y range, so it is a
two-handed prop split across both hands, not a mirrored pair. What it does confirm is the
construction: **rigid single-bind, appended subindices, no skeleton change** -- which is what four of
the five weapons here already did, and which Mythra now does too.

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

**Incineroar's red is a model variant, not a tint**, and the pair is now identified even though
the names are not. `_ZN3app26FighterSpecializer_Gaogaen11revenge_offERNS_7FighterEb` at
`main 0xab86f0` calls the module at `boma+0x150` through vtable `+0x50` with

    group 0xc8e6a844f (12 chars)   state 0x11a164aef7 (17 chars)   <- revenge OFF, in revenge_off
    group 0xc8e6a844f (12 chars)   state 0x1331d072dd (19 chars)   <- revenge ON,  at main 0xab91e0

Same group, two states, one per direction, so it is a two-way variant switch and not a one-shot.
The surrounding calls confirm the module layout is being read correctly rather than guessed:
`boma+0xa0` vtable `+0x270`/`+0x2b8` in the same function are `set_power_mul_5th` and
`set_reaction_mul_3rd`, which is exactly the Revenge boost this port already reproduces.

**All four hashes in that function are compile-time baked and cannot be recovered from the game.**
`0xc8e6a844f`, `0x11a164aef7`, `0x1331d072dd`, plus `0x1a264dbb63` (26) and `0x1658651362` (22) on
the adjacent calls, are in none of the three sources. Searched for directly: they are not mesh
names, material labels, `mesh_object_group` names or `mesh_object_full_name`s in his
`model.numshb`/`numdlb`/`numshexb`/`numatb` (138 candidate names, none 12 characters), not strings
in `model.nusrcmdlb` or `model.xmb`, and his `model/body/c00/model.nuanmb` is 104 bytes with no
visibility tracks at all. This is native C++, so the compiler hashed the literals at build time
and the strings are simply not in the binary.

What his model DOES carry is a `_001`/`_002` pair of both `alp_gaogaen` and `skin_gaogaen`
materials, which is the variant the switch selects between. Pikachu has no second material set, so
a tint remains the only available approximation and the orange pulse above is it. Resolving the
names would not change that — it would only make the mechanism quotable.

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

**STEP 2 HAS NOW RUN, AND IT FAILS.** Pikachu does not load with the cloned article appended. The
game crashes on entry to the training arena, `work/crash-0917b.log`:

    Type:           Data Abort
    Address:        0000000000000138
    Fault Address:  0000000000000000
    Thread Name:    TaskWorker2
    ReturnAddress[06]:  smashline + 0x2a68c (fighter_initialize_object_data + 0x2c)

Every other frame in that trace is `cross2_Release.nss` -- the game's own weapon-creation path,
entered through smashline's fighter-init hook. Byte-for-byte the same signature as the two crashes
in early September that were blamed on the clone and then excused.

**What makes this one a result rather than a third anecdote is that the excuses are all gone.**
Each of the three things that stood between the clone and a verdict has been eliminated and
verified separately:

| suspected confounder | state now |
|---|---|
| zero-byte `d03speciallwstart.nuanmb` on the card | repaired; `deploy.sh` verifies by size, 98/98 |
| `share-to-added` hanging every load | fixed at b2469ae; 0 aliases, all 12 textures shipped |
| spikeball assets never shipped (`gen-article-assets.sh`'s own diagnosis) | 24 files shipped AND all 24 declared |
| a `vl.prc` declaring a param object with no weapon | retired from the card |

and the control is not one boot but forty-two: `work/pt2.log` through `work/pt40.log` all load with
`CLONE_ENABLED=false`. Clone off, 42 of 42. Clone on, 0 of 10, three of them crashing at the same
address.

So the clone is the cause, and `gen-article-assets.sh`'s header is wrong where it says the missing
assets explain the crash -- they are present now and it still crashes.

### What was ruled out next, and what is left

`clone_weapon`'s second parameter is named `original_article_id`, and the obvious reading is that
we pass the wrong thing: we pass `WEAPON_KIND_PACKUN_SPIKEBALL` (`0x1E8`, 488) while
`FIGHTER_PACKUN_GENERATE_ARTICLE_SPIKEBALL` is `0x0`. **That reading is wrong.**
`vendor/sl_api.rs:325` uses it as `LOWERCASE_WEAPON_NAMES.get(original_article_id)`, and that table
is `StaticArrayAccessor::new(0x5186bd0, 0x267)` -- 615 entries, while `WEAPON_KIND_*` runs 0..615
across 616 constants. It is indexed by weapon KIND, so `0x1E8` is correct and the `.unwrap()` on
line 325 legitimately succeeded. The call is right.

What the append actually does (`sl_weapons.rs:151-175`) is copy Piranha Plant's article descriptor
verbatim into a new list, leak it, and point Pikachu's `static_article_info` at it. The descriptor
is found -- a miss panics with "Failed to append article table" and no panic appeared -- so the
fighter data is built, and the null deref is in the game's USE of it.

`NEW_AGENTS` being empty was the next guess and it is also wrong. `sl_api.rs:358` pushes the agent
unconditionally, keyed on the weapon kind, carrying `owner_id`, `owner_name_ffi` and
`new_name_ffi`; `use_original_code` is stored on it, not a condition for it. So the three
`weapon_*_hook` rewrites have their data and the path-name rewriting is not the hole.

**What the fault address says, and it narrows this a long way.** `Address: 0x138` with
`Fault Address: 0x0` is a read of `something->field_0x138` where `something` was NULL -- an
absent OBJECT, not a bad function pointer, which an instruction-fetch abort would look different
from. So the game asked for the weapon instance behind this descriptor, got nothing back, and read
a field off it anyway. That is a plausible consequence of the one hole already written down in
`gen-article-assets.sh`: a cloned article resolves under the OWNER's costume slot, and while
`fighter/pikachu/{model,motion}/spikeball/c00` now ships, **vanilla has no
`fighter/pikachu/spikeball` at any slot** and there is nothing to fall back to.

The Kirby path in the same file is the useful contrast. At `sl_weapons.rs:365` it builds
deliberately DISABLED descriptors -- `max_count: 0`, `on_init_callback` and `on_fini_callback`
both transmuted from `0u64` -- for articles Kirby must not have. So the descriptor's two callbacks
are function pointers the game will call, and the fighter path copies Piranha Plant's verbatim.
Whether they are packun-specific is not yet established and is worth establishing, because
`fighter_initialize_object_data` is exactly when an `on_init_callback` would fire.

`update_weapon_count` is not the answer on its own, which is worth recording since
`src/articles/mod.rs` named it as the next thing to read. There are two consumers.
`sl_weapons.rs:145` runs while `new_descriptors` still holds only the fighter's ORIGINAL articles
-- the clone is pushed afterwards at line 153 -- so it cannot touch a cloned article's
`max_count`. The second, `sl_weapons.rs:378`, does run after the pushes and would work, but it is
in the Kirby-copy function, not the one that appends our article.

**The next step needs no console.** The frames above `[06]` are all `cross2_Release.nss`, which is
`main`, and the stack trace gives module-relative offsets -- `+0x33a07bc`, `+0x3a640c`,
`+0x3af458`, `+0x6875c4`, `+0x3a761c`, `+0x6081fc`. `bash tools/decomp.sh main 0x33a07bc ...`
reads the game's own weapon-creation path and shows what returns null.

### The three articles are blocked, not cancelled

Ptooie, the rope, the parachute and the dragon hand all need a real article, and the mesh graft
cannot substitute for any of them. `the-ptooie.md` holds the whole transcription source -- 28
functions, status map, confirmed hashes -- and none of it is wasted; it is just not writable into
a shipping move until an article can exist. `CLONE_ENABLED` is `false` again so the mod loads.

If Pikachu loads with the spikeball appended, all four constants exist and the remaining work is
three rows in `tools/gen-article-assets.sh` and three `clone_weapon` calls:

| article | weapon kind | donor's own id | assets in the arc |
|---|---|---|---|
| Ptooie’s spiked ball | `WEAPON_KIND_PACKUN_SPIKEBALL` | `0x0` (packun) | 14 model, 10 motion |
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

### The clone's crash, root-caused to a named singleton

The faulting instruction is `main + 0x3476e44`, from `PC` in the crash report -- not from the
stack trace, whose entries are RETURN addresses and so decompile as partial functions. Read flat:

    0x3476e20  sub  sp, sp, #0x40             <- function entry
    0x3476e30  adrp x8, $+0x1e46000
    0x3476e34  ldr  x8, [x8, #0x3b0]          <- a global pointer, at 0x52bc3b0
    0x3476e38  ldr  x20, [x8, #0x1228]        <- x20 = that object's field 0x1228
    0x3476e3c  ldr  x19, [x1, #0x20]
    0x3476e40  ldr  x0, [x19, #0xb0]
    0x3476e44  ldp  x8, x9, [x20, #0x138]     <- FAULTS: x20 is NULL

and `0x52bc3b0` is, at offset +0x0 exactly,

    _ZN3lib9SingletonIN3app21FighterParamAccessor2EE9instance_E

So the crash is in the fighter PARAM accessor. What it does next is walk `x20+0x138` as a vector
of 56-byte elements -- `ldp` the begin/end pair, subtract, `asr #3`, multiply by
`0x6db6db6db6db6db7`, which is division by 0x38 -- and iterate them at `0x3476e94`. The object at
`instance_+0x1228` that owns that vector was never created.

The call chain, each frame resolved to its containing symbol rather than guessed:

    [06] smashline + 0x2a68c   fighter_initialize_object_data + 0x2c
    [05] create?               + 0x84c
    [04] EnableOffset          + 0x37c
    [03] InitializeModules     + 0xe04
    [02] InstantiateBattleObject + 0xe68
    [01] Initialize            + 0x1bc
    [00] (no symbol)

That is fighter module initialisation reaching into the param accessor, which is consistent with
the one thing a cloned article demonstrably lacks: a param object. `vl.prc` is where a fighter
declares its article params, and this mod does not ship one -- it is in `deploy.sh`'s RETIRED list
precisely because it "declared a 21st article param with no weapon behind it", and
`gen-vlprc.sh`'s override is disabled because it null-dereferenced in smashline's
`load_fighter_effects`. With the clone on, Pikachu has seven articles and nothing declares the
seventh.

`src/articles/mod.rs` already names the API for this and calls it unnecessary: "`get_param_reimpl`
FALLS THROUGH to `original_lookup_param` on a miss, so a missing `param_spikeball` does not crash"
and "`smashline::add_param_object` is the supported way to whitelist a param object ... if those
numbers ever turn out to matter." The fall-through claim is about a param LOOKUP at runtime and
says nothing about the accessor's own construction at fighter init, which is where this dies.

**Two single-variable experiments, in this order, neither yet run.** Both are one line and both
need one boot:

1. `smashline::add_param_object("pikachu", "spikeball")` before the `clone_weapon` call.
   `sl_api.rs:268` pushes onto `WHITELISTED_PARAMS`, and `sl_params.rs:328` already treats a
   fighter with new articles the same as one with whitelisted params -- so this may change
   nothing, and finding that out is worth one boot because it is the cheapest of the two.
2. `use_original_code: false`. That stops smashline loading Piranha Plant's fighter module and
   running the spikeball's own code, which is what pulls Packun-specific machinery into a match
   with no Piranha Plant in it. It costs the article's free behaviour -- the arc, the juggling,
   the damage -- so it is only worth it if 1 fails, and it turns Ptooie into a projectile that
   has to be written rather than inherited.

**Do not run these while nobody is watching the console.** A crash on entry to a match is not
destructive, but it leaves the card in a state where the mod cannot be played, and the whole
point of keeping `CLONE_ENABLED = false` is that everything else in the moveset works.
