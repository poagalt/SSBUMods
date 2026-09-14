# The visuals: what is grafted, what cannot be, and what the rope would cost

Written 2026-09-14 on the instruction to "add all the correct visuals ... behavior should be the
same as the current chrom sword implementation".

## The mechanism, and why it is not an article

`tools/merge-weapons.sh` merges each source fighter's weapon into Pikachu's body model as a named
mesh object, bound to a bone grafted under his own `HaveR`, and `src/sword/mod.rs` shows it for the
motions that need it. Technique from LilyLavender Modding Workshop episode 39.

This deliberately avoids `smashline::clone_weapon`, which crashes inside the game's own
`fighter_initialize_object_data` — the Ptooie blocker in `handbook/open-questions.md`, with
`CLONE_ENABLED = false` in `src/articles/mod.rs` to prove it was tried. A merged mesh needs none of
that machinery: it is part of Pikachu's model, it inherits his skeleton's animation for free, and
a visibility flag is its entire lifecycle.

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
with `tools/arc-fetch.sh`. Textures are still not shipped: twelve aliases in `tools/gen-config.sh`
point Pikachu's paths at each source fighter's vanilla bytes.

**It used to be one sword for four fighters.** `SWORD_MOTIONS` pointed seven motions at
`chrom_sword`, so Cloud's back-air, Mythra's dash attack and her tilts all swung Chrom's Falchion.

## What a one-bone graft cannot do

The graft carries ONE bone. That is exact for a weapon that is a stick — the bone is the handle and
Pikachu's hand animation moves it correctly. It is wrong for an article whose own skeleton
animates:

    gaogaen/rope        RopeC RopeL1..3 RopeR1..3 Throw_TR    the cord bends
    gamewatch/parachute Parachutea1 a2 b1 b2                  the canopy opens
    kamui/dragonhand    Hand1..4 HandD HandU                  the hand morphs

Those shapes come from each article's own `model.nuanmb`, and nothing in this port plays an
article's animation. Grafting them shows rigid geometry frozen in one pose. For Corrin's up-air
that is merely wrong, so it keeps no weapon rather than the wrong one. For the rope it is worse
than nothing: a straight bar between Pikachu and wherever the victim happens to be.

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

That is also the answer to "this problem may have its roots in the article issue". It does. But the
route to a real article is `clone_weapon`, and `clone_weapon` crashes. The mesh graft cannot
substitute, because the rope's whole job is to be a moving link between two objects.

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

**Also found and not fixed:** the native Revenge boost is applied through
`AttackModule::set_power_mul_5th` and `set_reaction_mul_3rd`, both of which `revenge_off` resets to
1.0 at `[boma+0xa0]` vtable 0x270 and 0x2b8. This port uses neither — `grep` finds no call — so the
boost reaches the damage some other way. Worth an audit of its own.
