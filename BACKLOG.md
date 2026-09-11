# Backlog

**Authoritative.** Every outstanding item and planned future item for this repo lives here.
If it isn't in this file, it isn't tracked.

> **This front section was badly stale and has been rewritten.** It carried every Tier 1, 2 and 3
> item unchecked while the work was done and verified, and it still said extraction needs the game
> running after the full c00 dump had made that false. That is not a cosmetic problem: a backlog
> that lists finished work as open is a backlog that gets ignored, and it nearly caused a second
> port of the animation set. The historical sections below are left as written -- they record how
> each thing was found -- but this section is now the state of the mod.

---

## Where the mod actually is

Everything below is verified by `tools/check.sh`, which passes in full. The numbers are from its
own output, not from memory.

| Area | State |
| --- | --- |
| Moves ported | Every slot except neutral B. D-throw is deliberately vanilla Pikachu; nothing else is. |
| Hitbox positions | 36 moves, 452 hitbox-frames, every one within 0.00 world units of its source. |
| Hitbox radii | Every one exact, top- and bone-anchored, under the hypothesis in `topscale.rs`. |
| Animations | 74 shipped, 72 scale-compensated; 31 synthesised motion_list entries. |
| Animation coverage | 0 gaps in both directions: no replaced move plays a Pikachu animation, and no source variant in a ported family is unreproduced. |
| Damage and knockback | 38 moves, 3146 non-geometric macro arguments, every one identical to the source's -- including `ATTACK_ABS`, where the four cargo throws keep theirs. |
| Motion entries | 77 motions, 385 fields: blend, FAF, intangibility window, `no_stop_intp` and the flag set all match the source. |
| Hand-written specials | 83 source params across four objects: 25 used with their values checked against the game's prc, 58 declared `NOT MODELLED` with a reason, 0 unaccounted. |
| ACMD reachability | 71 of 71 registered scripts reachable from a motion entry. |
| Status kinds | 7 new kinds at 0x1F0-0x1F6, starting exactly at Pikachu's own NUM of 0x1F0; RAT_STATUS_KIND_NUM is 0x1F7. Verified loading on hardware. |
| Attributes | `fighter_param.prcxml` carries 23, `fighter_param_motion.prcxml` 14, both UTF-8. |
| Deploys | 86 files, every one verified by size on the console, truncated uploads repaired. |

### Playtest round 1 (build e2ec6ad): it loads, and five things were wrong

Pikachu loads. `motions 77/77 resolved`, `status_kind_count=0x1f7`, no crash on entry -- so the
architecture holds and the zero-byte animation really was the load-crash cause.

Five reports, four root-caused from the log and the crash report, all fixed:

| Report | Cause | State |
| --- | --- | --- |
| Up-B turns Pikachu invisible | Synthesised entries kept the TEMPLATE's `scripts`, so `special_hi_open` named `expression_specialairhi1` -- Pikachu's own Quick Attack script, which calls `set_whole(false)`. 38 of 77 entries affected. | Fixed in synth.rs, gated by check 3k |
| Cargo throw froze the game | `check_button_on` is HELD, so the still-down grab button committed a throw on the first carry frame -- inside the engine's THROW status, which owns the motion. Null deref in `change_motion`. | Fixed: trigger not held, and the carry waits for THROW to end |
| Revenge banks a stack but the hit still lands | Refunding the damage a frame later does nothing about the reaction already in flight. | Fixed: `DAMAGE_NO_REACTION_MODE_ALWAYS` armor for the window |
| Revenge deals its damage with no freeze | Never implemented. | Fixed: 14 frames at 1/8 speed, length approximate and logged |
| Whiffed whip runs the escape animation | Waited for `is_end` on a 117-frame animation, then played 44 more. Incineroar's own entry says a whiff costs 64. | Fixed: exits at the FAF, read at runtime |

Still open from that round: **the Alolan Whip does not grab.** Not fixed, instrumented. The log
shows `RAT_SPECIAL_S_SHOULDER` never appears, and only `CatchModule::is_catch` was consulted. It
now acts on `is_catch`, `GrabModule::is_grab` or a valid capture id, and logs all of those plus
the grab collision's actual radius and the nearest opponent's distance every frame the window is
open -- which separates "the ACMD never ran" from "nobody was in range" from "it connected and we
asked the wrong question".

Two measurements worth keeping from that log:

- **`PostureModule::scale` reads 1.0, not 0.96.** It is a runtime multiplier, not the fighter
  `scale` param, so it does not settle the bone-radius question in section 3 -- but it does mean
  the self-test's "expected 0.96" note is comparing against the wrong thing.
- The up-B rises 74.98 world units and its status chain is exactly right, which is how the
  invisibility was traced to the motion entry rather than to the move.

### Playtest round 3 (build 2e8065d): three fixed, one reframed, one still open

| Report | What the log said | State |
| --- | --- | --- |
| Cargo throw crashes in the same place | Byte-identical crash, and `librat_of_doom` is in no frame and not in the module list. CARGO logged nothing -- because the sink TRUNCATED the log on every launch, so the crashed session's evidence was erased by the next boot. | Log now rotates to `.prev`. The remaining outside-reach the module has, pushing the victim into DK's SHOULDERED statuses, is off behind `TELL_VICTIM_SHOULDERED` |
| Whip does the rope throw against nobody | `is_grab` goes true on motion frame 17 -- one frame after `set_rebound(true)`, one BEFORE `macros::CATCH` runs -- while `cap` stays `0x50000000`, which is BATTLE_OBJECT_ID_INVALID. It also takes an index the call never passed. | Removed from the trigger; both indices plus `is_attack_occur` now logged across the whole window |
| Revenge damage lands before the hit registers | No freeze existed, and the fire is on script frame 8 of `special_lw` regardless. | Freezes for `floor((absorbed*0.65+6)*1.5)` frames first -- Ultimate's hitlag formula times Incineroar's own `special_lw_hit_stop_mul` |
| Revenge sometimes banks a stack and still takes the hit | Log shows `st=72 damage_air_1` at the counter, so the reaction happened anyway. | `set_no_reaction_damage_power(9999)` added alongside the mode, which may not carry through bindgen's `_address: u8` |
| Up-B invisible | Not reported this round; the synth fix worked. The guard still fires, but `get_whole` returns u64 and the `== 0` test may simply be wrong. | Guard now logs before AND after, which settles it in one line |

Confirmed from the constant table rather than assumed: `SWING_GAOGAEN_CATCHED` is 0x176, inside
the common range that ends before Pikachu's own extras at 0x1E1, so the victim status is
reachable. And smash-script's `CATCH` passes its 11th argument straight through as that status,
so the ACMD call is structurally correct. Whatever stops the grab is not the script.

Also measured: `PostureModule::scale`, `base_scale` and `owner_scale` all read exactly 1.0. They
are the runtime multiplier, not the `fighter_param` scale -- so they neither confirm nor refute
the bone-radius decision in section 3.

### What the next round needs

The card holds the current build and `tools/deploy.sh` reports DEPLOY OK. One training-mode load,
then:

```bash
bash tools/analyze.sh
```

That pulls and resolves the telemetry, checks the log's BUILD stamp against HEAD before anything
else (a playtest of the wrong build is worse than no playtest), summarises the self-test, prints
every move's probe lines, and mirrors any new crash report with its full return-address chain.

**Worth doing in that session, in this order:** side-B next to an opponent (the whip probes only
log while the catch window is open), a cargo grab and each of the four throws, an up-B, a Revenge
against something slow, and a run-in Revenge plus a rising one so `moved x` and `peak` answer the
momentum question in section 4.

---

## Open work

### 1. Neutral B / Ptooie -- the last unported slot

STEP 1 IS ANSWERED. Playtest round 1 loaded cleanly with the clone off and the card repaired, so
the zero-byte animation really was the load-crash cause and the article clone is no longer even
the leading suspect -- both crashes it was blamed for had a truncated `.nuanmb` present at the
same time. What is left is step 2: flip `CLONE_ENABLED`, rebuild, deploy, load once. Two minutes,
and it decides which of the two Ptooie routes to build. Full reasoning in `pikachu-moveset/plugin/src/articles/mod.rs`.

If the clone works, the remaining Ptooie work is scoped and small on the ball's side -- its six
weapon ACMDs are one hitbox each (14 damage, angle 55, KBG 80, BKB 50, size 5.0; 18 damage on
Shoot) -- and a new state machine on the fighter's side: six statuses per situation
(Start / Wait / ShootF / ShootB / End / Failure), driven by holding and tilting, plus a `SEARCH`
box so either player can hit the ball. The fighter-side ACMDs are nearly trivial: Start sets a
generate flag at frame 10, ShootF/B call `ArticleModule::shoot_exist`, End calls `remove_exist`.

If it does not, the fallback is Pikachu's own `DENGEKIDAMA` (article 0x0, free because Thunder Jolt
is the move being replaced) with the spikeball's model shipped under
`fighter/pikachu/model/dengekidama/c00/`. No registration, no load-time risk, but the ball's whole
lifecycle has to be hand-written.

### 2. Questions one playtest answers

The self-test (`plugin/src/telemetry/selftest.rs`) and the per-frame telemetry already log
everything these need. Read `<sd>/rat-telemetry.log` over FTP afterwards with
`bash tools/ftpq.sh get /rat-telemetry.log`.

- **Did every motion load?** `SELFTEST ... motions N/77 resolved`, with any failure named. All 77
  are confirmed present in the shipped `motion_list.bin`, so an UNRESOLVED line is a console-side
  failure.
- **Is the object scale really 0.96?** `SELFTEST ... scale=` reads it from the running game. Every
  hitbox radius and top-anchored offset in the mod is corrected against that number and it has
  only ever been asserted.
- **Did the 7 new status kinds register?** `SELFTEST ... status_kind_count=`, read from
  `global_table[0xC]` where smashline writes it. Expect >= 0x1f7.
- **Do the attribute changes apply?** `PARAMS` vs `EXPECTED` on the first frame of each fighter.
- **Does the engine scale root motion by the fighter scale param?** `moved x` per motion. Little
  Mac's f-smash `Trans` runs 0 -> 17.77 model units; compare `attack_s4_s` against 19.72.
- **Kazuya's f-air forward push.** Neither script has a movement call and `move` is clear on both
  motion entries, so it is neither ACMD nor root motion. Read `moved x` for `attack_air_f`.
- **Training Modpack save-state crash.** The dead `param_spikeball` article param and the
  `FLAG_PAUSE` leak, its two best candidates, are both fixed. Re-test.
- **Does Revenge keep any momentum?** `moved x` on `special_lw_start` after running into it, and
  `peak` on `special_air_lw_start` after rising into it. Incineroar keeps 60% and 70%; this port
  stops dead. See section 4.

### 3. Bone-anchored hitbox radii -- a decision that can be falsified in one look

`SCALE_BONE_RADII` in `tools/reach/src/bin/topscale.rs` is set on the object-scale hypothesis, with
the reasoning written out there. Training Modpack's hitbox visualisation settles it: put modded
Pikachu's f-smash beside Little Mac's own and compare each fist bubble against its owner. Both
states are verified to build clean and it is a one-line flip.

### 4. Source behaviour the hand-written specials do not reproduce

`tools/check-srcparams.sh` enumerates all of it and the modules carry a `NOT MODELLED:` line for
each with the reasoning. In rough order of what a player would notice:

- ~~**Revenge hard-stops on entry.**~~ DONE. `START_X_MUL` 0.6 and `AIR_SPEED_Y_MUL` 0.7 are
  applied to the entering speed in `speciallw/mod.rs`, on MOTION energy with the control energy
  cleared. The playtest observable stands as a check: telemetry logs `moved x` and `peak` per
  motion, so running into a grounded Revenge should now carry 60% of the speed rather than
  planting.
- ~~**No re-grab protection on Alolan Whip.**~~ NOW NATIVE, and it took no new code.
  `FighterStatusCapture_set_invalid_capture_SwingGaogaen` in lua2cpp_common reads
  FIGHTER_STATUS_SWING_GAOGAEN_WORK_INT_INVALID_CAPTURE_FRAME off the VICTIM, and the victim's own
  catch status copies that from the captor's `special_s_invalid_capture_frame` -- which is 60 for us
  now, through `src/params`. It needed the capture to exist before any of it could run; see
  `specials::take_captive`.
- **Revenge does not build from shield damage.** `special_lw_revenge_shield_damage_mul` is 0.2.
- **The carry does not speed Pikachu up -- BUT THE STATED REASON WAS WRONG.** This said the
  multipliers have to be applied in place and would compound to 1.1^n across status changes, so it
  needed a per-status-change hook. Donkey Kong does not do it that way at all:
  `FighterSpecializer_Donkey::apply_lift_param` -- exported, and already called by
  `shoulder::begin` -- sets the COMMON instance flag FIGHTER_INSTANCE_WORK_ID_FLAG_LIGHT_WEIGHT
  (0x20000058, index 88, inside Pikachu's 232) and the engine reads the flag. Nothing compounds
  and nothing needs a hook. What is NOT established is whether that flag alone reproduces
  `param_lifting_walk`'s 1.1x, since `apply_lift_param` reads no params itself. **The observable
  is the walk speed while carrying**, against Donkey Kong's.
- **The whip's bands are read as lines, not capsules.** Each is really `x1,y1 -> x2,y2` with a
  radius on `top`; this module reads only x1. It holds while `drive_victim` moves the victim along
  x only, and stops holding the moment the airborne whip lets them separate vertically.
- **The up-B's `stick_mul` (30) and `limit_y_mul` (0.5) are unmodelled.** Both are read by Game &
  Watch's C++, and the community reference port this module derives from does not model them
  either. The observable is how much horizontal control the rise has -- compare against Game &
  Watch directly before writing code for it.
- Smaller: `special_air_s_catch_speed_y_max` (0.05, vertical drift cap while holding in the air),
  `special_lw_hit_stop_mul` (1.5x hitlag on the counter hit), and
  reduced air control after the whip. (The lariat's 1x-2x motion rate is DONE -- see
  `specials::lariat_rate`.)
- Presentation, deferred with the rest of the visual layer: both KO cinematics, Revenge's emissive
  glow scaling and voice pitch.

**Not a gap:** three of the whip's four air params are identical to their grounded twins, so the
air whip sharing `START_SPEED_MUL` is correct rather than a shortcut.

### 5. Still genuinely open

- **Soft-platform carry.** DK's cargo carry does not use `_PASS`, so carrying an opponent through
  a soft platform behaves as Pikachu's own movement does rather than as DK's.
- **`arc-raw` full `data.arc` pull.** Abandoned at 0.59 GiB and resumable. Not needed now that the
  c00 extraction covers every fighter's models, skeletons and animations -- 40,300 files locally,
  with no console involved.
- **Kirby's copy ability.** Neutral B becomes Ptooie, which is an article.
  `smashline::whitelist_kirby_copy_article` exists; decide what Kirby inherits once Ptooie works.
- **Pikachu's orphaned articles.** Thunder Jolt and Thunder are unreachable now that neutral B and
  down B are replaced. `lockout/mod.rs` bounces their statuses; the articles themselves remain.
- **Rename.** "Rat of Doom" is a placeholder. The CSS rename ships and is verified as UTF-16LE
  with all five labels.

### 6. Deferred by design

The whole visual layer beyond animations: taunts, victory animations and theme, voice clips and
SFX, portraits and stock icons, the boxing-ring title. Per-source appendages beyond Chrom's
Falchion (Mac's glove, K. Rool's foot, Kazuya's leg) and their hurtboxes. No reflector, and no
K. Rool belly armor -- both deliberate, see `docs/decisions.md`.

### Known risks

- **ARCropolis 4.0.9 is pinned to Smash 13.0.5.** A console update breaks the whole toolchain.
- **No local test path.** Nothing runs in-game on this machine; the console is the only target.
- **sys-ftpd truncates roughly one upload in eighty to zero bytes** with a clean return code. The
  size-based verifier and repair pass in `tools/deploy.sh` are load-bearing because of it.

---

## Added after the George teardown

### Tier 1 additions

- [ ] Patch Pikachu's `motion_list.yml` — **FAF lives there as `cancel_frame`, and intangibility
      as `xlu_start`/`xlu_end`**, not in the plugin. Parity on those axes is impossible without it.
- [ ] Fetch `ParamLabels.csv` from `ultimate-research/param-labels` so `fighter_param.prcx` can be
      authored by name rather than by hash.
- [ ] Adopt the **`sys_*` effects-only rule** — using any fighter-prefixed effect reintroduces a
      CSK dependency. This is the mechanism, not a style preference.
- [ ] Do **not** empty `model.nuhlpb`. George does, to stop its rig fighting borrowed animations;
      we drive Pikachu's own animations and need his helper bones intact.

### New risks

- [RETRACTED] ~~Sword show/hide has no precedent.~~ It does — LilyLavender Modding Workshop ep. 39 "Adding A Sword" toggles Chrom.s Falchion with `set_mesh_visibility`, and George is built on that recipe. George cannot toggle only because he reused an existing mesh-object name. Old text: George.s appendage is permanently attached and always
  visible; its plugin imports **zero `ModelModule` symbols**. Toggling mesh visibility per-move is
  the first place this project leaves the proven path.
- **Runtime scaling has no precedent either** — same finding, same cause. The Incineroar b-throw
  scaling plan is unproven.
- Appendage technique that *is* proven: extra sub-meshes under an existing mesh-object name,
  single-bound to the item-hold bones (`HaveR`/`HaveL`), new material in `model.numatb`, vanilla
  skeleton untouched.

---

## Added after round 4

- [ ] **Rename from the placeholder "Rat of Doom"** — touches the mod folder, crate name,
      `info.toml` display name, and the plugin's agent registration.
- [ ] **CSK-requiring variant with full source effects.** ADR 0002 forbids fighter-prefixed
      effects in the main mod. A second build that *does* require the CSK Collection could use
      each move's real visuals (Cloud's sword trail, Mac's punch flash). Separate deliverable,
      not a fix to the main one.
- [x] **Trip attack** — assigned to Bowser, matching the get-up attack. Was a third floor-adjacent move and
      is still unassigned. Get-up is Bowser, ledge is Ganondorf.
- [ ] Runtime scaling, moved here from Tier 1. **Note: the George mod is not a source for this
      technique** — its plugin imports zero `ModelModule` symbols and does no runtime model
      manipulation of any kind. What George *does* prove is the appendage attachment method
      (extra sub-meshes on the item-hold bones). A different reference is needed for `set_scale`.

---

## Added after the sword research — three risks retired, one added

**Retired:**

- ~~"Sword show/hide has no precedent."~~ LilyLavender Modding Workshop **episode 39** does
  exactly this — Chrom's Falchion on Little Mac, toggled with `ModelModule::set_mesh_visibility`.
  George is built on that recipe; it cannot toggle only because it reused an existing
  mesh-object name instead of a distinct one.
- ~~"Blender is a hard blocker for appendages."~~ `ssbh_data_json` / `ssbh_data_py` round-trip
  `.numshb` to JSON. Merging a sword into a body model is a JSON edit.
- ~~"No `data.arc` is a wall."~~ ArcExplorer + the `arc-network` Skyline plugin extract over the
  network by IP. Atmosphère and Skyline already run on the target console.

**Still true:** Blender *is* still required for the animation pass. Retargeting a humanoid
animation onto Pikachu's quadruped skeleton is a different problem from merging a static mesh,
and Tier 1 remains script-first.

**Added:**

- [ ] **Pikachu's `model.adjb` (168 KB) must stay consistent after any mesh merge.** Only Chrom,
      Cloud and Pikachu have body adjacency buffers; George never had to handle this.
      `ssbh_data_json` handles `.adjb`.
- [ ] Correct `reference-recipe.md`: George binds to **five** points (`HaveL`, `HaveR`, `HandR`,
      `ShoulderL`, `ShoulderR`), not two.
- [ ] Console extraction session: Chrom's `fighter/chrom/model/sword/c00/` **and Pikachu's
      vanilla `fighter/pikachu/model/body/c00/`**. The Pikachu body model is the real forcing
      item — no download substitutes for it.

---

## Added after the API research

- [ ] **Read George's actual source** — `github.com/LilyLavender/ssbuGeorgeMoveset`. The whole
      reference-recipe was reverse-engineered from the binary; the source is public and makes
      that unnecessary. Verify `reference-recipe.md` against it.
- [ ] **Evaluate `WuBoytH/SSBU-Dumped-Scripts` as the primary parity source.** It is the vanilla
      ACMD for every fighter, article and item — the actual script, not a hitbox table. Likely
      better than rubendal for armor and intangibility windows, which was the one gap in the
      Round 3 plan. Falls inside the already-approved "public datamined data" decision.
- [ ] Pull episode 39 (`ssbu-mod-workshop/episode39`) as the sword template and episode 46
      (`ArticleCloningGuide.md`) as the article-clone template.
- [ ] Fetch `ParamLabels.csv` **and** `articles/Labels.txt` from `ultimate-research/param-labels`.

**Risk downgraded:** `ModelModule::set_scale` and `set_joint_scale` are confirmed to exist in the
`lua_bind` surface. Runtime scaling still has no worked example in a shipped mod, but it is no
longer speculative — only unproven.

---

## Sword confirmed into Tier 1

- [x] Sword returns to **Tier 1**. Both reasons for deferring it (needs Blender, no precedent)
      were false. Path: prove the pipeline with Byleth `swordflare` (already on disk), then ship
      Chrom's real Falchion by the episode 39 recipe.
- [ ] Our plugin must not be named `plugin.nro` — George already occupies that filename in the
      console's Skyline plugins folder.
- [ ] Verify the Ken-Sugimori-sourced `pikachu_c00_model.nusktb` against a vanilla dump. A
      restyle should not touch the skeleton, but this is currently an assumption.

---

## Status after the build session

**Shipped and installed (tag `v0.2-attrs-motion-upb`):**

- [x] 22 conventional moves, frame data copied verbatim from vanilla ACMD
- [x] 18 attributes via `fighter_param.prcxml` on Pikachu's row (index 7)
- [x] `motion_list.bin` patch: FAF and intangibility for 41 entries, **including Fox's dodges**
- [x] 10 synthesized motion entries, which is what unblocks specials generally
- [x] Game & Watch up-B (Fire + parachute)
- [x] Bone-parity harness (`tools/check-bones.sh`)

**Not shipped, with reasons:**

- [ ] **Down B — Revenge.** Counter detection, 3-stack storage, 60 s timer and the
      damage/knockback multiplier are fighter-specific status logic, absent from the ACMD dumps.
      A hitbox-only port would pose without countering. Needs status-script authoring.
- [ ] **Side B — Alolan Whip.** Command grab with three branching follow-ups that holds the
      opponent. Needs new motion entries (synth handles that now) plus grab-state logic.
- [ ] **Neutral B — Ptooie.** Needs a new article; `smashline::clone_weapon` plus the episode 46
      recipe is the path.
- [ ] **F-throw — DK cargo.** Motion entries are synthesized (`throw_f_{f,b,hi,lw}`), but the
      carry state machine and its walk/jump/turn are still to write.
- [ ] **The sword.** Pipeline is understood and Chrom's Falchion is extracted; the mesh merge
      into Pikachu's body via `ssbh_data_json` has not been done.
- [ ] **Runtime scaling** for Incineroar's b-throw. Tier 2.
- [ ] Recalibrate the four remapped hitbox offsets for Pikachu's proportions.
- [ ] Little Mac's angled f-smash variants — no motion entries on Pikachu; could be synthesized
      the same way as the specials now that the technique works.

## Added after the sword merge

- [x] Chrom's Falchion merged into Pikachu's body — `tools/merge-sword.sh`. Mesh `chrom_sword`
      (444 verts, 1764 indices, transferred exactly), a `Sword` bone appended under `HaveR`
      (bone 22 of 77 → 78), material `pasted__met_chrom_001`, and a `numshexb` group so the
      blade is in the draw/shadow table. All five model files round-trip cleanly.
- [x] Textures are **not** redistributed: `config.json` declares the three paths with
      `new-dir-files` and points them at Chrom's vanilla textures with `share-to-added`.
- [x] Little Mac's angled f-smashes, now that motion synthesis works.

**Open on the sword:**

- [ ] **Calibrate the blade's orientation.** Its world rotation is Pikachu's `HaveR` composed
      with the inverse of Chrom's hand rotation, which is not guaranteed to look right.
      `SWORD_TRANSFORM` in `tools/merge-sword.sh` is the single knob. Needs eyes on it.
- [ ] **Costume 0 only.** Each costume has its own model files; c01–c07 keep vanilla geometry
      and simply will not show the blade. The moveset itself is agent-wide and unaffected.
- [ ] Tighten visibility to hitbox-active frames. Currently the blade shows for the whole
      motion, which reads better than popping in and out but deviates from the brief.
- [ ] Verify `share-to-added` resolves a *vanilla* source path. If the blade renders untextured,
      ship the three `.nutexb` files instead — a one-line change in `merge-sword.sh`.

## The four remaining features — what actually blocks each

**Neutral B — Ptooie.** Blocked only on **asset extraction**. The recipe is
`smashline::clone_weapon` plus `ssbu-mod-workshop/episode46/ArticleCloningGuide.md`, and
Pikachu is confirmed cloneable. Needs `fighter/packun/model/<article>/` and
`fighter/packun/motion/<article>/`, which require `arc-network`, which requires the game
running. Ready to go the moment there is a window.

**Down B — Revenge.** Blocked on **novel implementation, no reference exists**. Marth's counter
ACMD only toggles `FIGHTER_MARTH_STATUS_SPECIAL_LW_FLAG_SHIELD`; the detection is engine-side and
keyed to fighter-specific statuses Pikachu does not have. HewDraw-Remix, the largest open-source
moveset project, only *tweaks* vanilla counters — `bayonetta/src/status/speciallw.rs` overrides
`Pre` and nothing else. So adding a counter to a fighter without one means writing the
`StatusLine::CheckDamage` protocol, a new status kind, damage nullification and the buff system
from scratch. `AttackModule::set_power_up` / `set_reaction_mul` are the right APIs for the
multiplier half. Plausible failure mode is a soft-lock, so this needs in-game iteration, not a
blind write.

**Side B — Alolan Whip.** Command grab with three branching follow-ups that holds the opponent.
Needs catch and throw statuses plus the opponent's `gaogaen_special_s_thrown_*` motions, which
*are* present on Pikachu. Motion synthesis handles the caster side.

**F-throw — DK cargo.** Motion entries `throw_f_{f,b,hi,lw}` are synthesized and carry DK's real
FAF. What remains is the carry state machine: `catch_cargo_wait` and its walk/jump/turn, which
DK does not name that way — the real entry names still need to be found with `tools/dump.exe`.

## What would unblock the most, in order

1. **A window with Smash running** — extracts Piranha Plant's article files, and gives
   `cargo skyline listen --ip <console-ip>` for live panics. Everything above benefits.
2. **One test session** with the current build, to confirm the up-B works and calibrate the
   sword's orientation.

## Ptooie — unblocked on assets

Extracted during the game-running window: `fighter/packun/model/spikeball/c00/` (12 files:
2 mesh objects `spike_all1Shape` + `ballShape`, an 8-bone skeleton `Root/Trans/Rot/Rot2` plus
four `Collision` bones, one material `metal_spikeball_001`, three textures) and
`fighter/packun/motion/spikeball/c00/motion_list.bin`. Also grabbed `vl.prc` for packun,
gaogaen and donkey. `model.nuanmb`, `main.nuanmb` and `update.prc` do not exist for this article.

- [x] Assets staged under `fighter/pikachu/{model,motion}/spikeball/c00/` and declared in
      `config.json` `new-dir-files`.
- [x] `smashline::clone_weapon("packun", *WEAPON_KIND_PACKUN_SPIKEBALL, "pikachu", "spikeball",
      true)` with `FIGHTER_PIKACHU_GENERATE_ARTICLE_SPIKEBALL = 0x7`. Pikachu has exactly seven
      vanilla articles, so 0x7 is the first free index.
- [ ] **Nothing spawns it yet.** Piranha Plant's `special_n` status must be ported for the ball
      to be spat out. Pikachu has `special_n`/`special_air_n`; packun uses `special_n_start` and
      friends, so motion synthesis is needed on top. This is the same untestable status work as
      Revenge, and it is now the *only* thing between here and a working neutral B.
- [ ] Possible `vl.prc` entry for the article. Packun's `vl.prc` is extracted; whether spikeball
      needs params is unverified.

## Conflict with "Ken Sugimori style Pikachu"

Both mods ship `fighter/pikachu/model/body/c00/model.{adjb,numatb,numdlb,numshb,numshexb,nusktb}`
— six files. Introduced by v0.3, the first version to touch Pikachu's model. Resolution chosen:
disable the Ken Sugimori mod by prefixing its folder with a dot, which ARCropolis skips.

## Ptooie caused a hard crash — article clone reverted

Enabling `clone_weapon` for the spikeball article crashed the game on fighter load, reliably.
Three Atmosphère crash reports, all Data Abort with fault address `0x0` on thread `TaskWorker2`:

```
ReturnAddress[06]:  (smashline + 0x2a68c) (fighter_initialize_object_data + 0x2c)   x2
ReturnAddress[00]:  (smashline + 0x1769c) (load_fighter_effects + 0x48)             x1
```

Cause is named in episode46's own guide: *"if you're cloning something like Mario's Fireball, or
any other article that calls to a fighter specific effect, you need to copy said effect to the
agent you're cloning to."* Ptooie's spikeball references packun effects and nothing under
`effect/fighter/pikachu/` provides them, so smashline dereferences null while loading them.

**Reverted:** the `clone_weapon` call is commented out and the article's assets are parked at
`pikachu-moveset/staged-spikeball/` instead of shipping. Console files removed too.

**This creates a real conflict with ADR 0002.** Making the clone work means shipping
`effect/fighter/packun/...` under `pikachu`, and a fighter-prefixed effect archive is exactly the
CSK dependency the brief forbids. Options, none free:

1. Ship packun's effect archive anyway and accept a CSK-style dependency for neutral B only.
2. Find whether the spikeball article can be cloned with effects stubbed out — unknown whether
   smashline tolerates a missing effect set.
3. Drop Ptooie and pick a neutral B whose article has no fighter-specific effects.
4. Keep Thunder Jolt.

Needs a decision before any further work on neutral B.

## Ptooie crash — root-caused and fixed without CSK

The crash was **not** about effects, despite `load_fighter_effects` appearing in one of the three
reports. Reading smashline's own source settled it: `src/params.rs`, `init_fighter_p_object`
builds its allowed-name list as

```rust
allowed_names.push(Hash40::new(&format!("param_{}", agent.new_name)).0);
```

and reads it out of the **host** fighter's `vl.prc`. So a cloned article named `spikeball`
requires a top-level `param_spikeball` object in Pikachu's `vl.prc`.

Verified directly: `param_spikeball` = `0xf679aaf13` is present in packun's `vl.prc` and was
**absent** from Pikachu's, while all seven of Pikachu's own articles have their `param_*` entry.
`clone_weapon` whitelists the name automatically but finds nothing to read — hence the null.

**Fix:** `tools/prc-tool/src/bin/vlmerge.rs` copies that one param object across
(6,385 → 8,438 bytes, 20 → 21 entries) and the mod ships the merged `vl.prc`. No effect archive
involved, so **the no-CSK constraint holds** and option 1 was never needed.

Guarded by a new harness check: if an article clone is installed without `param_spikeball` in the
shipped `vl.prc`, `tools/check.sh` fails and `deploy.sh` refuses to deploy.

**Ball visibility:** its appearance comes from the model files, which ship. Without packun's
effect archive the particle work will not play, but the ball should still have a physical form.

- [ ] **Unverified in game.** Needs a training-mode load to confirm the crash is gone.
- [ ] Nothing generates the article yet — neutral B still needs packun's `special_n` status
      ported. That is unchanged by this fix.

## Reach: root-caused with measurements

`tools/reach` does forward kinematics over a source fighter's skeleton and animation, so a
hitbox's actual world position can be computed frame by frame. Two reported symptoms turned out
to have **different** causes, and my original single explanation ("bone-local offsets don't
reach") was wrong about the mechanism.

### Little Mac f-smash: lost root motion

His f-smash animation translates the root **21.45 units forward** — he lunges. Pikachu's f-smash
animation has **no root-motion track at all**. Measured `Trans` z displacement:

| frame | Little Mac | Pikachu |
|---|---|---|
| 8 | 5.47 | — |
| 14 | 15.91 | — |
| 30 | 17.71 | — |

The arm itself is *not* the problem: with root motion excluded, Mac's `armr` at frame 14 sits at
z=4.75 and Pikachu's at z=5.04 — nearly identical. All the missing range is the lunge.

Root motion across the other ported moves, so the scope is known:

| slot | source | pikachu | delta |
|---|---|---|---|
| f-smash (Little Mac) | 21.45 | 0.00 | **+21.45** |
| dash attack (Mythra) | 29.70 | 48.02 | −18.33 (travels too far) |
| f-tilt (Mythra) | 3.67 | 0.00 | +3.67 |
| jab, u-tilt, u-smash, n-air | ~0 | ~0 | none |

That matches the playtest exactly: only f-smash was reported short, and only f-smash loses
meaningful root motion.

### Falco u-tilt second hit: bone in a different place

Its three hitboxes anchor to `shoulderl`/`arml` at frame 12. Falco's `arml` is then at
**y=19.15** — swung high, it is an anti-air. Pikachu's `arml` is at **y=0.88**, by his feet. The
hitbox spawns roughly 18 units too low. It does come out; it is nowhere useful.

### The decision this forces

Fixing reach by re-anchoring to `top` at the measured world position works, and the numbers are
in hand (scale ratios: Mac 1.11/0.96, Falco 1.05/0.96, Pikachu 0.96). But absolute reach on a
much smaller fighter puts hitboxes far from the body — Falco's u-tilt hit would sit ~21 units
above a Pikachu who is a fraction of that tall, with nothing drawn there.

This is exactly what the brief's "port in the appendage for the period the hitbox is active"
anticipated. Options:

1. Apply exact reach now; accept invisible hitboxes far from the body until appendages exist.
2. Apply exact reach only where it stays near the body; scale the rest to Pikachu's proportions.
3. Reproduce root motion (give Pikachu Mac's 21-unit lunge) rather than offsetting hitboxes —
   faithful for f-smash specifically, and it moves the hurtbox too, which is how the real move
   trades range for commitment.
4. Do the appendage work first so exact reach has something drawn behind it.

## Reach applied: option 3 for f-smash, option 1 elsewhere

**`tools/reach/src/bin/reanchor.rs`** re-anchored **34 hitboxes** from limb bones to `top`, at the
world position they occupy during the *source* fighter's animation, scaled by
`source_scale / 0.96`. Limb-anchored hitboxes went 39 → 5; the remaining 5 are Little Mac's
f-smash across its three angles, which keep `armr` deliberately.

Representative corrections:

| move | bone | was (bone-local) | now (top-relative) |
|---|---|---|---|
| Falco u-tilt 2nd hit | `arml` | (0,0,0) at y≈0.9 on Pikachu | y=20.95 |
| Falco u-tilt 2nd hit | `arml` | (6.2,0,0) | y=28.90 |
| Fox u-smash | `kneer` | (0,1,0) | y=19.95 z=4.63 |
| Corrin u-air | `haver` | (0,8,-1) | y=25.80 z=8.93 |
| Kazuya f-air | `toel` | (0,0,0) | y=7.44 z=11.19 |
| Chrom jab | `haver` | (0,0,8.4) | y=21.92 z=3.83 |

**`tools/lunge.sh`** reproduces Little Mac's f-smash lunge with
`SET_SPEED_EX(agent, 1.8, 0.0, *KINETIC_ENERGY_RESERVE_ATTRIBUTE_MAIN)`. His own ACMD contains no
speed call at all — the movement is pure animation root motion — so it has to be synthesised.
1.8 is a **first estimate**: ground friction decays the value, so actual displacement will be
less than 1.8 × frames.

Telemetry now logs `moved x` per motion, so the lunge can be calibrated against Mac's measured
17.7 world units by frame 14 rather than guessed at again.

### Confidence, honestly

The vertical corrections are trustworthy — Falco's u-tilt going from y≈0.9 to y≈21 matches the
observed symptom exactly, and Fox's u-smash and Corrin's u-air land at plausible anti-air heights.
The **lateral (x) components are less certain**: hitbox offsets are model-space while the kinetic
API treats x as forward, and Mythra's f-tilt second hit came out at x=−10 to −17 with y near or
below zero, which looks wrong. If f-tilt's second hit misbehaves, that is the first thing to
re-examine.

- [ ] Calibrate the f-smash lunge speed from the logged `moved x`.
- [ ] Re-examine Mythra f-tilt's second-hit x/y.
- [ ] Appendages next, per the agreed order.

## Playtest round 2: one fix confirmed, one regression, one wrong claim

**Falco u-tilt: confirmed improved** by re-anchoring. Its hitboxes ride a real limb.

**Palutena n-air: regressed.** Range went from roughly correct to tiny. Cause: her 11 hitboxes
anchor to `haver`, the **item-hold bone**, which stands in for a staff Pikachu does not have. Her
staff hitboxes ring *her* body; transplanted at absolute world positions they formed a shell at
y=13-22 around a fighter a fraction that tall, so nothing near Pikachu was covered.

That gives the rule the first pass was missing, and the two results split on exactly this line:

| anchor | meaning | absolute reach |
|---|---|---|
| real limb (`armr`, `arml`, `kneer`, `toel`, `shoulder*`) | an extended body part | **correct** — keep re-anchoring |
| `haver` / `havel` | a weapon the host does not have | **wrong** — leave bone-anchored until appendages exist |

`reanchor.rs` now excludes the item-hold bones. Re-anchored count 34 → 12: Falco u-tilt 6,
Fox u-smash 4, Kazuya f-air 2. The 22 weapon-anchored hitboxes (Palutena n-air, Mythra f-tilt,
Corrin u-air, Chrom jab) revert to their previous behaviour and are the appendage work's job.

**Mythra f-tilt does NOT have a second hit — I was wrong.** Hitbox IDs 0-3 are created on frame
10 and *re-created with the same IDs* on frame 11, which repositions the sweeping sword;
`clear_all` follows on frame 12. One hit, active f10-11. My "second hit" wording came from
counting ATTACK calls rather than reading the IDs.

**The lunge did not work, for a mundane reason.** The speed was applied before the first
`frame()` call, i.e. frame 1, but a smash attack holds its charge at frame 5 and does not swing
until frame 14 -- the speed had decayed long before. Now applied at frame 8 and cleared with the
hitboxes. A second bug: the frame number was hardcoded to `8.0`, which silently skipped the
down-angled variant that starts on frame 9. All three angles now carry it.

`tools/build.sh` runs generate → re-anchor → lunge → compile in order, since running the passes
by hand in the wrong order produces silently wrong output.

- [ ] Calibrate lunge speed (3.0 estimate) against the ~11.6 world units Mac covers f8-f14.
- [ ] Verify Fox u-smash and Kazuya f-air, which were re-anchored but not yet reported on.

## Capsule hitboxes: the unified fix for "too small" / "strange"

Playtest: Fox u-smash "too small", Palutena n-air "strange, different from before", Kazuya f-air
unclear, **f-smash travels correctly** (lunge confirmed at +10.89 to +19.44 against Mac's real
17.7-23.8).

Measured cause. Bind-pose head heights, via `tools/reach` against an empty animation:

| fighter | head y | pikachu/source |
|---|---|---|
| **Pikachu** | **7.20** | — |
| Fox | 11.84 | 0.609 |
| Falco | 13.87 | 0.520 |
| Little Mac | 14.62 | 0.493 |
| Mythra | 14.98 | 0.481 |
| Corrin | 16.25 | 0.443 |
| Palutena / Chrom | 17.48 / 17.50 | 0.412 |
| Kazuya | 17.87 | 0.403 |

Pikachu is **40-60% the height** of every source fighter. A straight absolute transplant put
Fox's u-smash hitboxes at y=17-24 while Pikachu's head is at 7.2 — a ~10-unit dead zone in which
nothing near him was covered. That is "too small". Palutena's n-air failed identically, forming a
shell above him.

**Fix: capsule hitboxes**, using the game's own stretched-hitbox feature — the
`Some(x2), Some(y2), Some(z2)` arguments to `ATTACK`, which ROB's d-tilt already uses. Each
re-anchored hitbox now runs from the position scaled to Pikachu's proportions (where a
Pikachu-sized version of the move would hit) out to the source's absolute position (exact reach).
Body coverage and true range at once.

This also **retires the limb-vs-item-hold split**. Excluding `haver` was a workaround for the
dead zone, not a fix; with capsules the weapon-anchored moves get their near coverage back too.
Re-anchored count is 34 again, now all as capsules, and none had to be skipped.

## Vacuum: what vanilla Pikachu behaviour still leaks

`tools/motion-patch/src/bin/leaks.rs` resolves the `game_script` each patched motion names and
checks whether the plugin overrides it. **The first version reported a false clean of zero
leaks** — it resolved hashes in reverse, and the label dictionaries contain motion names but not
`game_*` ACMD names, so everything looked unresolvable and was skipped. Fixed to hash forward.

Real result: 13 leaks, narrowed to 10 after porting Fox's dodge scripts, and of those:

- `attack_s4_hold`, `attack_hi4_hold`, `attack_lw4_hold` — **not real.** No source fighter has a
  `game_` charge script and Pikachu's contains zero commands.
- `catch_wait`, `catch_pull`, `catch_cut` — no source script either.
- `escape`, `escape_b` — Fox has no `game_escape`/`game_escapeb`; Pikachu's are empty.
- `throw_f`, `throw_lw` — **intended.** DK cargo is unimplemented and d-throw is deliberately
  Pikachu's.

**Ported as a result:** Fox's `game_escapeair`, `game_escapeairslide` and `game_escapef`, which do
exist and were being shadowed by Pikachu's. 37/37 registrations now reachable.

### Root motion leakage, measured

| slot | source | pikachu | leak |
|---|---|---|---|
| dash attack (Mythra) | 29.70 | 48.02 | **+18.33 — travels too far** |
| f-smash (Little Mac) | 21.45 | 0 | −21.45, now covered by the lunge |
| f-tilt (Mythra) | 3.67 | 0 | −3.67 |
| u-tilt (Falco) | 0 | 0.50 | +0.50 |
| jab, u-smash, n-air, aerials | ~0 | ~0 | none |

The `move` flag in `motion_list` controls whether animation root motion applies, and we copy the
source's flag onto Pikachu's animation. It is `true` on dash attack, both f-tilts, u-tilt, d-tilt,
b-air, dash grab, b-throw, ledge attack and both rolls — so Pikachu's own root motion is live on
those.

- [ ] **Dash attack over-travels by 18.33 units.** Fix is to clear `move` and drive the 29.7 via
      script, as done for f-smash. Not yet applied.
- [ ] f-tilt is missing 3.67 units of forward motion (minor).

## Endlag: cancel_frame IS being honoured -- do not "fix" it

The original report was that move lag felt "bounded by the animations of Pikachu's normal
attacks". I was close to applying a global `FT_MOTION_RATE` retime to force animation length to
match source FAF. **The telemetry says that would have been wrong** and would have corrupted the
frame data to chase a visual artifact.

Observed motion durations against the patched FAF and Pikachu's animation length:

| move | source FAF | pikachu anim | observed durations |
|---|---|---|---|
| jab (Chrom) | 23 | 29 | **22**, 29, 29 |
| u-tilt (Falco) | 30 | 36 | **29**, 36 x5 |
| f-tilt (Mythra) | 37 | 45 | **41**, 44, 45 |
| d-smash (G&W) | 34 | 73 | 8, **40**, 40, 54 |
| u-smash (Fox) | 56 | 56 | 56 x3 |
| d-tilt (R.O.B.) | 15 | 37 | **27**, 33, 37 |

The *minimum* duration tracks the source FAF, not the animation: jab interrupted at 22 against a
FAF of 23 and a 29-frame animation; u-tilt at 29 against FAF 30 and a 36-frame animation. If the
animation were governing, no duration could fall below it. D-tilt's 27 against FAF 15 is
plausibly human reaction time -- acting on frame 15 needs deliberate buffering.

**Conclusion:** frame data is correct and actionability follows the patched FAF. What is wrong is
purely *visual* -- Pikachu's animation keeps playing past the actionable frame, so the move looks
laggier than it is. That is an inherent consequence of the script-first decision and is fixed by
the animation pass, not by touching timing.

Movement measured this session, confirming the earlier root-motion table:

| move | moved x | expected |
|---|---|---|
| f-smash | **+9.62 avg** | lunge working |
| f-tilt | +0.00 | should be ~+4 (Mythra moves 3.67) |
| u-tilt, d-tilt, u-smash, d-smash | +0.00 | correct, no source root motion |
| dash attack | *not used this session* | over-travels by 18.33 when tested |

## Open after v0.11 (source animations, exact hitboxes)

### Verified done
- All 11 dropped-in moves measure **0.00 world units** of hitbox error against the source
  character, across 226 hitbox-frames, gated by `tools/check-hitboxes.sh` at 0.05.
- Hitboxes sweep along the source's real trajectory again, because they ride the source's bones.
- `Sword1` and `Stick` grafted on; Chrom's jab and Palutena's n-air no longer lose the sword's
  bind rotation and the staff's [0,-2.2,0] offset.
- Source fighters' own article ids and work-flag ids stripped. K. Rool's belly-armor flags were
  Pikachu's `FLAG_STRANS_1` and `FLAG_PAUSE`; Mythra's sword article id was his electro ball.
- `vl.prc` override retired while `clone_weapon` is disabled.

### To measure on hardware (telemetry already logs what is needed)
- **Does the engine scale root motion by the fighter `scale` param?** `anim-dropin.sh` leaves
  `Trans` untouched because animation data cannot answer this. Little Mac's f-smash Trans runs
  0 -> 17.77 model units. If the engine scales it, our travel is 17.77 * 0.96 = 17.06 against his
  17.77 * 1.11 = 19.72, i.e. 14% short, and `Trans` should be scaled by 1.15625 too. Telemetry
  logs `moved x` per motion -- compare `attack_s4_s` against 19.72.
- **Kazuya f-air forward push.** Reported as Pikachu's own animation pushing him forward, but
  neither Pikachu's nor Kazuya's `game_attackairf` contains a movement call and `move` is clear on
  both motion entries, so it is neither ACMD nor root motion. Read `moved x` for `attack_air_f`.
- **Do the attribute changes apply?** `dump_params` prints the live values against the expected
  ones on each fighter's first frame. Grep the log for `[rat] p0 PARAMS` / `EXPECTED`.
- **Hitbox radius.** `topscale.rs` deliberately does not scale `size`, because whether the engine
  folds the anchor bone's inherited scale into the radius is not answerable offline. Ours is
  currently 4-20% small on the scaled moves. If radius does NOT follow bone scale, scale `size`
  by the same ratio; if it does, bone-anchored radii are already right and only `top`-anchored
  ones need it.

### Still to do
- **Training Modpack save-state crash.** Best candidate was the dead `param_spikeball` article
  param, now removed; the `FLAG_PAUSE` leak is a second candidate, also now fixed. Re-test. If it

- **Does Revenge keep any momentum?** `moved x` on `special_lw_start` after running into it, and
  `peak` on `special_air_lw_start` after rising into it. Incineroar keeps 60% and 70%; this port
  stops dead. See section 4.
  persists: saving is read-only apart from a `sys_deku_flash` effect on `top` and a walk of
  article indices calling `get_article` / `remove_exist_object_id`, so the next suspects are the
  modified model files and the plugin's per-frame telemetry callback. Bisect by removing
  `librat_of_doom.nro` alone, then the mod folder alone.
- **Moves still on Pikachu's own animation**, so still subject to the bone-position mismatch:
  d-tilt (R.O.B.), d-smash (G&W), the angled f-tilt and f-smash variants, all four throws, grab
  and its variants, pummel, the three Bowser get-up/trip attacks, Ganondorf's ledge attack, and
  the Fox dodges. Each needs its `.nuanmb` extracted and added to `tools/anim-dropin.sh`, plus a
  row in `tools/moves.tsv` so the trace covers it. Extraction needs the game running (arc-network).
- **Ptooie / spikeball.** `clone_weapon` still crashes on fighter load and is disabled. Note that
  `FIGHTER_PIKACHU_GENERATE_ARTICLE_SPIKEBALL` was set to `0x7`, but Pikachu's vanilla article ids
  top out at `0x5` (MONSTERBALL) per skyline-smash's tables -- so 0x6 is the next free id and 0x7
  leaves a gap. Worth trying 0x6 before anything else.
- Revenge, Alolan Whip, DK cargo throws; sword orientation calibration; rename from "Rat of Doom".

## Open after v0.15 (full animation set, exact hitboxes, working attributes)

### Closed since v0.11
- **Attributes were never applying at all.** Shipped as UTF-16; ARCropolis parses prcxml with
  prcx + xml-rs, both UTF-8 only. Telemetry read every value as vanilla. Now UTF-8, and check.sh
  parses the file rather than checking its byte order mark (which was asserting the bug).
- **Landing, d-tilt, d-smash and 20 other animations** now come from the source fighter: 38
  shipped, up from 11.
- **Angled f-smashes** play Little Mac's own angled animations. They were up to 18.65 world units
  out of place, the worst error in the mod.
- **Two scale constants were wrong** (Cloud 1.03 vs 0.98, K. Rool 1.15 vs 0.89). All scales now
  generated from fighter_param.prc via each entry's own `fighter_kind` field.
- **motion_list.bin is built by tools/build.sh**, not by hand.
- **Root motion needs no correction.** Little Mac's f-smash travels +20.59 world units on
  hardware against his own 19.72, within 4.4%.
- **The aerial body lift is accepted by decision** -- see the ADR 0003 addendum.

### Blocked on the user
- **Training Modpack save-state crash.** Decoded: `Os { code: 3586 }` is FS module 2 description
  7, `ResultFsTargetLocked` -- `save_states.toml` is locked open by another handle while

- **Does Revenge keep any momentum?** `moved x` on `special_lw_start` after running into it, and
  `peak` on `special_air_lw_start` after rising into it. Incineroar keeps 60% and 70%; this port
  stops dead. See section 4.
  Training Modpack rewrites it. Every mod-side cause was eliminated: our plugin does no file I/O
  (Skyline has no SD log dir), no FTP server is resident during gameplay (the one answering is
  hand-launched ftpd, which exits on game launch), and saving is otherwise read-only apart from a
  `sys_deku_flash` effect on `top`. The file has been backed up to work/tm/ and renamed to
  `.bak` on the console so Training Modpack starts fresh. Awaiting a re-test.

### To measure on hardware
- **Hitbox radius.** `topscale.rs` scales offsets but not `size`, because whether the engine folds
  the anchor bone's inherited scale into the radius is not answerable offline. Ours is currently
  4-16% small on the scaled moves. If radius does not follow bone scale, scale `size` by the same
  ratio; if it does, only `top`-anchored radii need it.
- **Peak height.** Telemetry now logs vertical travel and peak per motion. Compare a short hop
  with an aerial against one without, to confirm the trajectory is unaffected by the drop-in.

### Still to do
- **Chrom's jab combo.** He has attack_12, attack_13, attack_100 and attack_100_end; Pikachu has
  none of them, so only jab 1 is ported. Needs synthesised motions with adopted animations, the
  same three-part recipe the angled f-smashes used.
- **Ptooie / spikeball.** `clone_weapon` still crashes on fighter load and is disabled
  (`CLONE_ENABLED`). Note `FIGHTER_PIKACHU_GENERATE_ARTICLE_SPIKEBALL` is set to `0x7`, but
  Pikachu's vanilla article ids top out at `0x5` per skyline-smash's tables, so `0x6` is the next
  free id and `0x7` leaves a gap. Try `0x6` first.
- Revenge, Alolan Whip, DK cargo throws (motions synthesised, animations not extracted).
- Sword orientation calibration; rename from "Rat of Doom".
- Dodge timing params (`escape_*_cancel_frame`, `escape_air_slide_speed`) still unshipped --
  `fighter_param_motion.prcxml` was never written. The dodge ACMDs and motion timings are in.

## Jab facing left: investigated, nothing found wrong (2026-09-10)

Reported as "jab doesn't work correctly when done to the left, it seems the hitbox still faces
right". Everything checked is exactly Chrom's:

- The ACMD is byte-identical to `lua2cpp_chrom/chrom/Attack11.txt`, all three hitboxes on
  `ATTACK_LR_CHECK_F` (top z=7.0, sword1 z=1.7 and z=8.4).
- Every bone in the `Sword1` chain -- Trans, Rot, Hip, Waist, Bust, ClavicleC, ClavicleR,
  ShoulderR, ArmR, HandR, HaveR, Sword1 -- is driven by the dropped-in animation, so no Pikachu
  bind offset leaks into the chain.
- The visible Falchion is fine. `Sword` (mesh) and `Sword1` (hitbox) have byte-identical bind
  transforms, and Chrom's `Sword1` track is a single constant frame, so both ride `HaveR`
  rigidly and cannot diverge.
- `Rot`'s rotation is the identity in every dropped-in animation, so nothing fights the engine's
  facing flip.
- Chrom's jab has no root motion at all (`Trans` z range 0.00) despite carrying the `move` flag.

The report almost certainly came from the partial deploy. Files upload in sorted order and the
run was cut off after 48 of 60, so `motion_list.bin` -- which sorts after every `cXX`/`eXX`/`fXX`/
`gXX` animation -- was among the twelve that never arrived. The console was running the new
animations against a stale motion_list. The card is now verified clean at 48/48.

**Needs one re-test before spending more on it.** If it survives a verified deploy, the next thing
to look at is Pikachu's jab status machinery rather than the move data: his `attack_11` loops back
to itself, whereas Chrom's transitions to `attack_12`, which Pikachu does not have.

## State at v0.17

### Confirmed working on hardware
- **All 18 movement attributes.** Telemetry from playtest-042639.log spans the v0.13 deploy and
  shows both states: weight 79 -> 135, walk 1.302 -> 1.575, dash 1.98 -> 2.42, run 2.039 -> 2.464,
  jump_y 35.5 -> 50.51, mini 17.12 -> 17.34, aerial 35.5 -> 50.51, initial 19.525 -> 27.781,
  air_x 0.957 -> 1.332, air_y 1.55 -> 1.63, dive 2.48 -> 3.097, gravity 0.095 -> 0.13,
  air_accel mul/add 0.09/0.01 -> 0.08/0.05. Every one matches the expected value exactly.
- **Root motion needs no scale correction.** Little Mac's f-smash travels +20.59 world units
  against his own 19.72, within 4.4%.
- 21 moves, 276 hitbox-frames, all 0.00 world units from the source character.

### Blocked only on the game being open
Run `grep -vE '^\s*#|^\s*$' tools/arc-pull/paths-round3.txt | tools/arc-pull/target/release/arc-pull.exe <console-ip>:43022 extracted`
with Smash running, then rebuild. That unblocks:
- **Dodge timing params.** getparam reports escape_air_slide_speed, _distance, escape_air_cancel_frame
  and every other escape_* param ABSENT from fighter_param.prc for both Pikachu and Fox, so they
  live in fighter_param_motion.prc. The dodge ACMDs and motion timings are already in.
- **Chrom's jab combo.** Needs c00attack12/13/100/100end, synthesised motions with adopted
  animations (the three-part recipe the angled f-smashes used), and
  StatusModule::change_status_request_from_script per workshop episode 10.
- Incineroar's Revenge, Game & Watch's up-B animations, DK's cargo throws.

### Needs one playtest before more work
- **Jab facing left.** Nothing found wrong; see the investigation above. The report coincided with
  a deploy cut off after 48 of 60 files, which left new animations against a stale motion_list.
- **Training Modpack save states.** `Os { code: 3586 }` is FS `ResultFsTargetLocked`. Every
  mod-side cause was eliminated. save_states.toml is backed up in work/tm/ and renamed on the card
  so it starts fresh.
- **Whether the aerials move Pikachu.** Telemetry now logs vertical travel and peak per motion;
  compare a short hop with an aerial against one without.

### Open, not blocked
- **Hitbox radius is 4-16% small on scaled moves.** topscale corrects offsets but not `size`.
  The better mechanism for the whole scale problem is `PostureModule::set_scale` on entering a
  ported move and back on exit: the fighter `scale` param is the "Character Size multiplier", so
  one call would make positions, bone-local offsets AND radii exact at once, replacing both the
  Rot-node animation edit and topscale. Not attempted blind -- the current approach is verified at
  0.00 on positions and switching mechanisms unverified could regress all 21 moves.
- **Ptooie.** Article id corrected to 0x6 (Pikachu's own run 0x0-0x5, so 0x7 left a gap). Still
  behind CLONE_ENABLED = false because it crashed on fighter load. Re-enabling also requires
  shipping vl.prc again with param_spikeball; check.sh 3b enforces that pairing in both directions.
- Sword trail (workshop episode 40) would restore what the AFTER_IMAGE strip removed.
- Alolan Whip; sword orientation calibration; rename from the "Rat of Doom" placeholder is now
  moot for the CSS but info.toml still uses it.

### Hazard worth remembering
tools/build.sh used to prepend w64devkit to PATH, which shadows grep, awk, iconv, od, xxd and tr
with minimal builds -- its grep returns 0 for any pattern containing "/". Any check using those
utilities could behave differently inside the build than outside. w64devkit now goes last.

## Corrections at v0.18

**The jab "flow regression" I recorded was wrong.** I wrote that Chrom's jab chains into jab 2 and
that porting it faithfully therefore left a jab which could neither repeat nor chain. Chrom has no
jab 2: `dump.exe extracted chrom attack_1` returns `attack_11` and nothing else, and the round-3
extraction confirmed it by failing to find c00attack12/13/100 in the archive at all. His jab is a
single slash, so our port matches him exactly and there is nothing to add. The claim came from
noticing he does not set `FIGHTER_STATUS_ATTACK_FLAG_ENABLE_RESTART` (Pikachu does) and inferring a
follow-up that does not exist.

**Animation filenames really are universal per motion.** Round 3 seemed to contradict it -- nine
requests came back Missing -- but that was me guessing prefixes (`h00speciallw`) instead of reading
them. Read from each source's own motion_list they are `d03speciallw*` and `d02specialhi*`, and
Pikachu's `special_lw` is `d03speciallw.nuanmb`: the same name as Incineroar's. The rule holds; I
just have to look the names up rather than pattern-match them.

**Palutena's e01throwf and e01throwlw were extracted unnecessarily.** The brief puts f-throw on
DK's cargo carry and leaves d-throw as vanilla Pikachu, so neither is needed. Harmless, but they
are in extracted/ unused.

**The George mod does not solve the up-B animations.** It ships no .nuanmb, no config.json and no
motion_list at all -- its Game & Watch up-B is script-only over Diddy's animations, exactly as ours
is over Pikachu's. So that gap is unsolved by the reference too, and the four G&W animations
(now extracted) would need the new-file recipe plus a motion_list repoint. Not attempted while
untestable: Pikachu's up-B is a teleport and G&W's is a parachute, so a wrong animation there
breaks the move rather than merely looking off.

### Closed at v0.18
- **Dodge frame windows.** Open since v0.2. They live in fighter_param_motion.prc, not
  fighter_param.prc. Diffing Pikachu's whole 132-param row against Fox's showed all FOURTEEN
  escape_* params differ, not the one or two expected: airdodge cancel 55 -> 40, airdodge stiffness
  50 -> 38, slide stiffness 84 -> 63 / 69 -> 54 / 109 -> 75, and the roll cancel and
  hit-normal frames each 1 to 3 frames apart. All now read from Fox and gated.
- **A verified inventory.** tools/inventory.sh reports, per slot, whether the script, effect
  override, animation and frame data are each actually present, checked against the shipped files
  rather than asserted. 25 of 25 ported slots are complete.

### Deliberately not taken from Fox
The same row diff showed the capture and grab offsets other fighters use to hold Pikachu
(ganon_special_*, pikmin_catch_*, ryu_final_*, butterflynet_*, snake_capture_*, wiifit_capture_*,
rockman_capture_*, gaogaen_rope_*, captain_special_hi_*, cloud_final_*, pickel_thrown_*) differ
too. Those describe where Pikachu's BODY sits inside someone else's move, so they stay his. Same
for the walk/run/item animation frame markers, `flip`, `motion_share`, `capture_motion_height`,
`cliff_hang_check_length` and `stop_ceil_speed_y_threshold` -- character-wide traits outside the
brief, which ADR 0001 excludes by decision.

## Status machines at v0.21

All four remaining brief items were attempted. Three are in; the fourth is gated.

### Revenge (down-B, Incineroar) -- implemented
Reimplemented, because it cannot be ported: SSBU counters are not in the ACMD dumps. The scripts
only raise a status work flag (Marth's FLAG_SHIELD frames 6-28, Incineroar's FLAG_STANCE 8-28) and
the detection is C++. smashline has no cross-fighter status sharing -- its share hooks cover the
four ACMD categories and nothing else.

Tuning is the game's own, from fighter/gaogaen/param/vl.prc param_special_lw[0], read with a new
tools/prc-tool prcdump: rate = min(1 + 0.5 + 0.075 * countered, 3.0), so 2.25x off a 10% hit, and
3.0 is the "stores x3" in the brief. Also found revenge_data, a 17-row power_mul/reaction_mul/
sub_sec table, which is why the boost drives reaction_mul as well as power_up.

Detection is an on_line(Main) watcher, not the status loop, because being hit moves the fighter
into a damage status -- the SPECIAL_LW loop stops running at the exact moment the hit lands.

### Alolan Whip (side-B, Incineroar) -- implemented
Feasible because the victim side is engine-global: CATCH names
FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED, and that whole family is in the COMMON status list.
Pikachu already carries `gaogaen_special_s_thrown_big` for being on the receiving end. Tuning from
param_special_s[0], 50 params.

**The input mapping is interpreted.** Which stick direction picks which of the three follow-ups is
not in the ACMDs, not in the params, and the code that reads it is C++. Up gives the shoulder toss,
down the lariat, otherwise the throw -- isolated in `chosen_followup` for a one-place change.

### DK cargo carry (f-throw) -- implemented as a STATIONARY hold
The grab and all four directional throws work, including the cargo up-throw. The carry locomotion
does not: DK's own side is eleven fighter-specific statuses (SHOULDER_WAIT, _WALK, _TURN, _JUMP,
_JUMP_SQUAT, _FALL, _LANDING, _PASS, _END...) which is an alternate movement mode in C++. The
one-status-with-phases approach does not apply, because the point of cargo carry is keeping normal
movement while holding.

Release valve: an input-less carry frees itself after 120 frames. A stuck grab would be worse than
the missing locomotion.

Outstanding: DK's own throw_f animation was not extracted before the game closed, so the cargo
grab runs his script on Pikachu's animation. One line in paths-round3.txt.

### Ptooie (neutral-B) -- gated, deliberately
Now `if CLONE_ENABLED` rather than commented out, so the call stays compiled and enabling is one
line. Article id corrected to 0x6 (Pikachu's own run 0x0-0x5, so the old 0x7 left a gap -- a
plausible cause of the load crash). Left off because it crashed on fighter LOAD, which blocks
testing every other move. Piranha Plant's ACMD is deliberately unported: without a working article
it would spawn nothing and make neutral-B worse than the Thunder Jolt it replaces. The three
enabling steps are written at the call site.

### Two build bugs these features exposed
- build.sh ran the PREBUILT tool binaries, so editing synth.rs did nothing until rebuilt by hand.
  An adopt_anim change silently no-opped and Revenge's stance kept the wrong animation. Tools are
  now rebuilt as step 0.
- That rebuild loop then aborted the whole build, because grep exits 1 when it finds nothing and
  `set -e` treats that as fatal -- and the abort looked like a clean finish.
- deforeign now also strips macro calls smash-script does not export, with the allowlist read from
  smash-script's own source so it cannot drift. Incineroar's shoulder toss calls
  CHECK_FINISH_CAMERA_IF_NOT_HP_MODE, absent from the 112 it provides. Audited: that one macro
  only, cosmetic, and none of the 21 traced moves reference any unsupported macro.

## v0.23 — the one wrong premise that caused five defects

**A ported move CAN have its own status kinds.** Everything below flowed from believing it could
not. smashline's `set_status_scripts` (vendor/sl_create_agent.rs:738, :802) reads the fighter's
status total out of `global_table[0xC]`, installs every registered script with
`sv_set_status_func`, tracks `max_new = max_new.max(status.id + 1)`, and writes that back. An id
past `FIGHTER_PIKACHU_STATUS_KIND_NUM` (0x1F0) is the documented path.

`src/specials/status.rs` declares seven at 0x1F0..0x1F6. Compile-time assert on the range,
check-status.sh gate on distinctness and on the deforeign KEEP list surviving.

### What that premise cost
- **Up-B**, twice over. First it kept Diddy's 0x1F0/0x1F1 and those were called "statuses that do
  not exist"; then v0.22 remapped them onto Pikachu's SPECIAL_HI_WARP and SPECIAL_HI_END. WARP is
  Quick Attack's second leg, and the port only registers Pre/Main/End, so Pikachu's own Init and
  Exec still ran — model hidden, warp displacement applied, direction guide spawned. Every
  symptom in one line: "teleport modified pikachu extremely high and/or turn him invisible" and
  "spawns quick attack arrows around if I change the direction of the stick". Diddy's *numbers*
  had been right all along.
- **Alolan Whip**, all of it. catch-remap.sed retargeted the command grab at CAPTURE_PULLED
  because Pikachu "has no equivalent of Incineroar's grabber statuses and none could be added" —
  and that remap is exactly the reported "teleports back to where he was, facing the other
  direction, and does the enemy freed from grab animation". Deleted. All seven SWING_GAOGAEN
  victim statuses (0x176–0x17C) are in the engine's COMMON list; the victim side never needed
  anything.
- **The five follow-ups** were crammed into one status with a phase variable and a recursive
  `pikachu_specials_main(fighter)` re-entry. They are five statuses now, as they are on Incineroar.

### The follow-up mapping is no longer interpreted
SmashWiki settles the rule: an attack button pressed after the grab but before the collision is
the back body drop; pressed just as they collide, the lariat; never pressed, both take 4%. And
which is which comes out of `param_special_s` — `special_s_lariat_offset_x` 24,
`special_s_lariat_end_offset_x` 10, `special_s_failure_offset_x` 6, matched by his
FLAG_LARIAT_AREA / FLAG_LARIAT_END_AREA / FLAG_FAILURE_AREA. 24 units is the lariat window because
24 / `through_target_return_speed` 2.8 = 8.6 frames and `special_s_lariat_hit_frame` is 9. The old
stick-direction reading is gone.

Also: the lunge runs on `FIGHTER_KINETIC_TYPE_MOTION` with `special_s_start_speed_mul` 1.3, not
UNIQ-then-GROUND_STOP. GROUND_STOP pinned him in place — "it also no longer travels anywhere".

### Revenge: the rules were half right
The tuning was already the game's own; the RULES were not, and SmashWiki's Revenge page has them.
The boost is **consumed by the next landed attack** (60 seconds is only the ceiling on an unused
one) — the 3600-frame timer with no consumption is "permanently stored the extra damage".
Counters stack. `cancel_percent` 36 is a loss condition on a stored boost, not a ceiling on
whether the counter fires; read the wrong way it also left the window stuck open after a big hit.
Being thrown drops it.

Active visual: an orange FLASH pulse plus a `sys_` aura followed on shoulderr/shoulderl/kneer/
kneel — the bone list is Incineroar's own `effect_speciallwrevenge`, minus `feelerca` which
Pikachu has no counterpart for. `gaogaen_revenge_aura` itself is CSK-blocked, but FLASH is what
Incineroar uses on this very move (`effect_speciallwstart` pulses FLASH purple), tints through
ColorBlendModule and needs no archive.

**The counter window is left alone.** Script frame 8 and Ultimate Frame Data's frame 3 agree:
`FT_MOTION_RATE(r)` is game frames per motion frame (smash_script's `FT_MOTION_RATE_RANGE`
computes `r = game_frames / motion_span`), so the stance's 0.25 runs motion frames 1..9 four times
faster than real time. The window really is ~25 real frames at the start of a long animation.
"I can only get it to trigger at the very start of the move" is Incineroar's behaviour.

### The vanilla-leak audit
- `attack_s3_hi` / `attack_s3_lw` still played Pikachu's own angled animations, because **Mythra
  has no angled f-tilt motions at all** — one animation for all three angles, which is why
  gen-moves.sh aliases both angled ACMDs to her `attacks3`. motion-patch gains a REANIM table
  pointing both entries at her `attack_s3_s` file.
- All four up-B motions kept PIKACHU's animations while taking G&W's `move` flag, so his Quick
  Attack root motion was driving a G&W move. They adopt `d02specialhi` / `airhi` / `hiopen` /
  `hiclose` now (shipped, declared in config.json, in the dropin pre-flight).
- `effect_specials` — Skull Bash's `pikachu_rocket_bomb`/`_aura`/`_jet` and a blue-green FLASH —
  is nulled. gen-moves.sh gains an `N` marker for nulling both scripts.
- **DONKEY was missing from deforeign's SOURCES** while gen-moves.sh had been porting five of his
  scripts, so `FIGHTER_DONKEY_STATUS_SHOULDER_START_FLAG_SHOULDER_WAIT` reached the built plugin
  unstripped. build.sh now diffs the two lists and refuses. A hand-maintained parallel list is a
  bug waiting for a new port.
- Audited and deliberately kept: the four `effect_landingair*` scripts are pure `sys_down_smoke`,
  and `effect_escapeb` is `sys_smash_flash` + `sys_dash_smoke`. Both are generic landing/roll dust
  every fighter has. `sound_*` and `expression_*` stay Pikachu's — porting them needs the source
  sound archives, which is ADR 0002 territory.

### deforeign gains KEEP_STATUS_IDS
The blanket strip is right about INSTANCE work ids and article ids. Status-SCOPED ids are
different: they index the region belonging to the running status, so they only collide with what
the HOST uses IN THAT SAME STATUS — and where every line of a status is replaced, the region is
ours. Stripping them threw away the only record of the move's timing (FLAG_THROW_ROPE on frame 37
IS the whip; LANDING_ENABLE and PARACHUTE_OPEN ARE the up-B's landing and parachute). Ten names,
with a three-point safety check written at the list.

### Cargo carry: the victim now gets the engine's statuses
`SHOULDERED_DONKEY_START` (0xC5), `SHOULDERED_DONKEY` (0xC6) and `_THROWN` (0xC7) are common, and
Pikachu already carries `shouldered_donkey` plus the whole `thrown_f_donkey_*` family. Nothing on
Pikachu reads DK's frame-16 SHOULDER_WAIT flag, so the victim sat in a generic caught state with
no status driving them — "freezes the opponent mid-air for a second instead of holding the enemy".
The carry requests the statuses directly. Still stationary; still needs DK's `e01throwf.nuanmb`.

### What is left, in order of what unblocks the most
1. **The article clone.** It is the only thing standing between the mod and both the visible ring
   ropes and Ptooie — `WEAPON_KIND_GAOGAEN_ROPE` 0x1BA and `WEAPON_KIND_PACKUN_SPIKEBALL`. It
   crashes on fighter LOAD, which blocks testing every other move, so it needs a test cycle of its
   own rather than riding along with anything else. Everything else in the moveset is now
   implemented, which makes this the right time.
2. **One extraction with the game running**: `fighter/donkey/motion/body/c00/e01throwf.nuanmb`,
   already listed in tools/arc-pull/paths-round3.txt. Ship it in anim-dropin AND add
   `("throw_f", "throw_f", "donkey")` to motion-patch's MAP, both together.
3. **Neutral-B is still Thunder Jolt** — the one slot with no port at all, waiting on (1).
4. Verify on hardware that status ids past 0x1F0 are safe. smashline's code says yes and nothing
   in the game's own tables is being resized by hand, but this is the first build to rely on it.
   If Pikachu crashes on selection, this is the cause, and the fallback is to own SPECIAL_HI_WARP
   and SPECIAL_HI_END completely by registering every status line rather than three.

## The full data.arc pull is abandoned, and why

**Do not retry this.** `tools/arc-pull/src/bin/arc-raw.rs` works and is correct; the transport
cannot carry it.

Two numbers killed it:

- **~2 MiB/s, and it is bandwidth, not protocol.** At 512 KiB blocks that is 263 ms per request
  with 5 ms of round-trip latency in it — 2%. A perfect zero-latency protocol would take it from
  1.90 to 1.94 MiB/s. This is the exact opposite of the per-file pull, where 98% of the time *was*
  round trips and a block cache bought 39x. 16.39 GiB at that rate is ~170 minutes.
- **The game crashes during sustained transfers.** Twice now. Once during three back-to-back
  full-table reads (faulted in `training_modpack + 0x6b180`), once 0.59 GiB into a sequential read
  ("the software was closed because an error occurred"; the client saw the socket die mid-read and
  exited cleanly with a resumable partial).

A 170-minute transfer that needs zero crashes, on a link that has produced two in an afternoon, is
not a plan.

Worth noting what did NOT crash: the filtered c00 pull, 40,264 files and 1.27 GiB in about seven
minutes. So sustained use is not automatically fatal — the difference is duration and that
`arc_network` opens its OWN `File::open("rom:/data.arc")` handle alongside the game's, and a long
sequential sweep appears to starve the game's own asset streaming. The earlier report's null deref
on MainThread in `cross2_Release.nss` with `arc_network`'s socket threads live fits that.

### What replaces it
Nothing, because nothing needs replacing. `extracted/` now holds every fighter's c00 animations,
skeletons, motion lists and params — 40,264 files, 1.27 GiB, pulled with zero failures — and a
targeted pull of any other subset takes minutes at 97 files/sec. The full archive was only ever
about never needing Smash open again, which is a convenience.

`arc-raw` stays in the tree. If the console is ever on 5 GHz or a USB-ethernet adapter, the
bandwidth objection disappears and it resumes from wherever it stopped.

### The nxdumptool route, for the record
Also dead, for a different reason: it needs `sdmc:/switch/prod.keys`, because the RomFS lives
inside encrypted NCA containers. That means Lockpick_RCM and a payload boot. The card's
`nxdumptool.nro` was already the right build (`nxdt_rw_poc v2.0.0 (rewrite-ade1a48)`, byte-identical
to the vendored copy), and the PC side is fully set up and smoke-tested in `tools/nxdt-host/` —
venv, pyusb, tqdm, libusb-1.0.dll — so if keys ever appear it is a five-minute job. Recommending it
without checking for the keys first was a mistake.

### arc_network is a boot hazard
Its `src/lib.rs:12` binds inside a `loop` and unwraps, so a held port panics the plugin at startup
rather than retrying — OS error 98 is EADDRINUSE. Killing a client mid-transfer is enough to
trigger it, and it then blocks launching the game at all until a full reboot. Once extraction is
genuinely finished for good, move `libarc_network.nro` out of the plugins directory.

## v0.25–v0.29: what the harness found once it was pointed at the specials

Pikachu loads on the vl.prc-reverted build, which settles the biggest open risk: **status kinds past
0x1F0 work on hardware.** Every design that depended on it — the up-B's two statuses, Alolan Whip's
five, the seven-status lockout — is sound.

### The specials had never been measured
`tools/moves.tsv` covered 21 moves. The mod replaces more. The up-B, Revenge and every Alolan Whip
hitbox were **outside the hitbox trace entirely**, so "all 276 hitbox-frames within 0.05 world
units" had never been a claim about the moves under active development.

Adding them: up-B **0.38** off, Revenge **0.44**, Alolan Whip's lunge/shoulder/whiff **0.29 / 0.24 /
0.45** — and the lariat, whose hitbox rides `arml`, **exactly 0.00**. Bone-anchored exact and
top-anchored wrong is the signature of `topscale` not running, and it was not: **every special was
missing from its MAP**, and the walk was one level deep so the hand-written up-B ACMDs two levels
down had never been opened at all. All ten are in MAP, the walk recurses, all measure 0.00. The
harness now covers 28 moves and 340 hitbox-frames.

Third instance of one pattern: a parallel list that must be kept in step by hand and is not.
`throw_f` outside the dropin pre-flight, `DONKEY` missing from deforeign's `SOURCES`, every special
missing from topscale's `MAP`. `tools/ported-motions.txt` now consolidates three of those lists.

### The recursion immediately broke something, and that is the useful part
`topscale` multiplies offsets in place and is **not idempotent** — safe only because `gen-moves.sh`
rewrites its targets from the vanilla dumps first. Recursing pulled a hand-written module into
scope and one build turned an offset of 6.0 into 6.2500005. The next would have compounded it,
with hitboxes drifting outward every rebuild and nothing visible in a diff of generated files.

Two fixes so it cannot recur: the up-B ACMDs are **generated** now (they were hand-placed only
because deforeign strips `FIGHTER_GAMEWATCH_*`, and `KEEP_STATUS_IDS` already preserves the three
flags they need), and topscale **refuses to write a file lacking the `@generated` marker**, naming
the file and the two ways out.

### Four more false cleans closed
The count is now nine on this project, and the shape is always identical: a check that reports
success on work it did not do.

- **`blend.rs` passed on zero work.** It took motions as command-line arguments and had never been
  given any, so it printed an empty table and "0 wrong" for as long as it had existed. Exits 2 now.
  With real input it found one defect in 48 motions: Palutena's `catch_dash` blends over 3 frames
  and ours snapped in over 0. `blend_frames` — how many frames the engine cross-fades INTO a motion,
  which is the seamless-transition requirement in numbers — had never been copied at all.
- **FK dumps were undeclared build inputs**, made by hand in earlier sessions with nothing
  regenerating them. `tools/gen-fk.sh` generates them from the local extraction.
- **`check-hitboxes.sh` failed on "no FK skeleton" while `trace-all` prints "no FK data"**, so a
  move with missing inputs silently stopped being measured instead of failing.
- **`gen-moves.sh` truncated itself.** Each move group is a double-quoted shell string; a comment
  containing a double quote ends it, and everything after is executed as commands. The only symptom
  was a stray `: No such file or directory`.

### Pikachu's own moves are now unreachable, not merely unused
`tools/motion-patch/src/bin/variants.rs` answers exhaustively what had only ever been answered by
accident. For each replaced slot: which motions does Pikachu have that the SOURCE lacks? Eight. Two
were the f-tilt angles (Mythra has no angled f-tilt at all). The other six are Skull Bash's
release/charge/weak, Thunder's hit reaction and Quick Attack's windup/recovery.

`src/specials/lockout/mod.rs` bounces all seven retired status kinds (0x1E1–0x1E7) to wait/fall and
logs loudly; sixteen leftover `game_`/`effect_` scripts are nulled. Two of those mattered beyond
cosmetics: `effect_specials` fires his rocket effects, and the Quick Attack effect scripts call
`VisibilityModule::set_whole` where only their own tails restore visibility — so reaching one and
leaving early strands him invisible for the stock. The up-B restores it on entry as insurance.

Neutral-B is deliberately excluded: Thunder Jolt is still his until Ptooie lands.

### Cargo carry moves now
DK implements it as eleven fighter-specific statuses with their own locomotion. Adding statuses is
possible, and reproducing his shape was still wrong: it means reimplementing walking, turning and
jumping and then hand-fitting Pikachu's speeds back in. Instead he stays in **his own** movement
statuses and the carry overrides only the pose, so the locomotion is his by construction. Attack,
special and guard transition groups are disabled every frame the carry is live.

**All nine carry animations were already on disk** — the first real dividend from the full c00
extraction. A feature filed as blocked on a console session was entirely local work.

### The infrastructure that made the rest possible
- **sys-ftpd was always running.** It is a boot sysmodule; the requests just lacked credentials, and
  FTP's `430` reads like a connection failure rather than an auth failure. Every "close Smash and
  relaunch ftpd" request in this project's history was unnecessary.
- **The deploy verifier was lying.** One run reported 64 files "remote absent" that had uploaded
  minutes earlier; a targeted `SIZE` query proved the card correct the whole time. sys-ftpd truncates
  LIST responses when busy — i.e. whenever Smash is running. Listings are a first pass now; anything
  suspicious gets a single-file query. Second time this file's own warning came true.
- Deploys skip unchanged files (a full run is ~10 minutes) and repair truncated uploads
  automatically. Verification remains the sole authority on success.
- **Telemetry writes to the SD card** as well as the TCP log, so a session can be played with nobody
  watching and read afterwards. The mount prefix is probed, not guessed.

### Still open
1. **Neutral-B (Ptooie)** — the only un-ported slot, blocked on the article clone. The mechanism is
   understood now from reading `init_fighter_p_object` rather than inferring: the param object must
   be in the host's own vl.prc, and `smashline::add_param_object` exists and was never tried. Two
   wrong claims in the old note are corrected. The decisive experiment is one deploy plus one
   restart, now that repairs eliminate the truncated-upload candidate.
2. **A playtest of v0.29.** Nothing here has been verified in motion.
3. Hitbox `size` scaling (4–20% small) — needs hardware to settle whether the engine folds in the
   anchor bone's inherited scale.
4. DK's `_PASS` (carrying through a soft platform).

---

## Round 5 (builds d03c990 -> b2a82ba): three crashes diagnosed from the game's own binary

This round is the first where the diagnosis came from reading the game rather than from
reasoning about it, so the entries below cite instructions rather than symptoms. See
`pikachu-moveset/docs/reading-the-games-code.md` for the four sources and
`docs/adr/0004-never-hand-a-victim-to-another-fighters-code.md` for the rule they produced.

### Fixed

| Symptom | Actual cause | Evidence |
| --- | --- | --- |
| Alolan Whip crashes at the rope throw | `status_SwingGaogaenThrown + 0xf8` is `LinkModule::get_parent_pos` followed by three unguarded loads. The parent is Incineroar's ROPE article. | Atmosphere report + `disasm` on `lua2cpp_common.nro`, instruction for instruction |
| Revenge does no fire damage | the counter requested `SPECIAL_LW` from inside `SPECIAL_LW`; the engine drops a transition to the current status, so no `main` ran and the hitbox never existed | `SpecialLwHit` exists as a real status in the decomp |
| Revenge lets the player slide | the entering speed went to `FIGHTER_KINETIC_ENERGY_ID_CONTROL`, which is the energy the STICK drives | `FUN_7100013e50`: `change_kinetic(UNIQ)`, `clear_unable_energy` twice, `set_speed` on MOTION |
| (not reported, found while reading) Revenge left Pikachu on 0.4x damage with no flinch, permanently | `End` did nothing; `SpecialLw_end` is three unconditional reverts | decomp, `SpecialLw/end.c` |
| (not reported) the whip could hold a victim who had mashed out | position was written every frame with no check that they were still caught | `status_SwingGaogaenCatched_Main` reads `get_clatter_time` and releases them itself |
| 25 of 30 added animations UNRESOLVED, nondeterministically | the directory was 38% over vanilla, almost all of it tracks for bones and meshes Pikachu does not have | 77 of 123 transform nodes foreign; growth 3673 -> 637 KiB and zero hitboxes moved |

### Open, instrumented rather than guessed

**The cargo throw crash.** An Instruction Abort with no symbol —
`rat_of_doom + 0x9cf8` under smashline's line hook with no engine frame between, which rules in a
bad code or object pointer and names nothing else. Four things changed that each remove a
candidate: the victim id is latched every frame it is valid instead of read at the instant
`throw_f` ends, a handoff with no valid victim declines, `hold_victim` leaves the position to
`CaptureModule` while the engine's capture still names us master, and every cross-object handle
goes through `crate::victim::boma`. Plus a crumb bracket: `SHOULDER START entered` before the
status shift and `START loop reached` after it, unbuffered, so the next report says which side of
`sub_shift_status_main` the game died on.

### Ready to implement, deliberately not started

**Carrying and jumping.** DK has eleven shoulder statuses and this port has six. The four missing
ones are read and the settings are in hand:

    ShoulderJumpSquat   SITUATION_GROUND  KINETIC_TYPE_MOTION       GROUND_CORRECT_GROUND
    ShoulderJump        SITUATION_AIR     KINETIC_TYPE_JUMP         GROUND_CORRECT_AIR
    ShoulderPass        SITUATION_AIR     KINETIC_TYPE_MOTION_FALL  GROUND_CORRECT_KEEP
    ShoulderJumpSquatB  (the reversed variant)

`FUN_7100022d00` is JumpSquat's main: `change_motion`, `StopModule::is_stop`, then a loop reading
`ControlModule::check_button_off` and `get_stick_y` against `get_param_float` — the ordinary
short-hop / full-hop decision — and on motion end it goes to SHOULDER_JUMP. `ShoulderJump_main_loop`
calls `FUN_7100022520`, the same throw-input helper the grounded states use, so **throws are
available in the air**, and falls through to SHOULDER_FALL on motion end.

Held because the carry itself still crashes. Four more statuses on top of a crashing feature is
the wrong order; the sequence is stabilise, confirm, then extend.

**Neutral B / Ptooie.** Unchanged: step 1 of the experiment in `src/articles/mod.rs` has now
passed many times over — clone OFF, card repaired, Pikachu loads and plays. Step 2 is one flag.
It is held for one reason only: if the clone does crash on load, nothing at all is testable, and
three fixes are waiting on a playtest. It goes first in the round after they are confirmed.

---

## Added after the native-call sweep

`tools/check-nativecalls.sh` compares the engine calls each source status makes against the calls
this port makes, across all 21 ported statuses, and reports 57 unmade ones. It is a reading list
rather than a failure list -- effects, sound and camera are deferred by design and counted rather
than listed. What follows is the triage of what is left.

### Fully researched, NOT implemented, and the single reason why

- [ ] **DK's carry CheckDamage — how an attack makes him drop the victim.** Donkey Kong keeps
      carrying through anything that is not a launcher, and this port has no CheckDamage line on
      its six carry statuses at all. The logic is decompiled and every constant in it is resolved:

      ```
      if !LinkModule::is_linked(boma, LINK_NO_CAPTURE)                  -> return false
      elif damage["owner_object_id_"] == get_node_object_id(LINK_NO_CAPTURE) -> return false
      else
          reaction = DamageTransactorImpl::calc_reaction_frame(damage["reaction_max_"])
          level = reaction <  get_param_float(boma,"battle_object","damage_level1") ? DAMAGE_LEVEL_1
                : reaction <  ...("damage_level2")                                 ? DAMAGE_LEVEL_2
                : reaction <  ...("damage_level3")                                 ? DAMAGE_LEVEL_3
                :                                                                    DAMAGE_LEVEL_FLY
          if level != DAMAGE_LEVEL_FLY
              if ItemModule::is_have_item(boma, 0) { ItemModule::drop_item(boma, 90.0, 0.0, 0) }
              return false
          else
              return true
      ```

      Everything needed is in hand. `damage_level1/2/3` are on the COMMON `battle_object` param
      object, so they read correctly off Pikachu with no help from src/params.
      `app::DamageTransactorImpl::calc_reaction_frame(float)` is EXPORTED, at 0x404ec0.
      `owner_object_id_` and `reaction_max_` came out of main's own strings via
      `tools/hash40.sh`. smashline supports the line (`StatusLine::CheckDamage`, typed
      `extern "C" fn(&mut L2CFighterBase, L2CValue) -> L2CValue`), and
      `lib::L2CValue__index_hash40` reads the table.

      **THE ONE OPEN QUESTION IS THE POLARITY OF THE RETURN VALUE**, and it is not settled by
      anything available here. `true` is returned only on the launcher path and `false` on every
      "keep carrying" path, which reads as "true means the damage applies". But the same shape
      returns `false` when DK is not carrying anyone at all, and "suppress damage whenever you are
      not carrying" cannot be right. HDR's `MewtwoThrown_check_damage` returns 1 with the comment
      "bypass release set speed/status", which points the other way again.
      `FighterStatusUniqProcessSwingGaogaenLariat_check_damage` looked like an oracle -- it returns
      true before the lariat connects and false after -- but it is a `FighterStatusUniqProcess`
      helper called from inside a status, not a registered status line, so it does not answer the
      question about lines.

      Guessing costs either immunity to damage while carrying or an instant drop, both of which
      are worse than the current absence, so ADR 0005 applies. **To settle it:** find the native
      caller of the CheckDamage line in `main` and read what it does with the result --
      `tools/disasm --dataxref` and the `FighterStatusModuleImpl` vtable are the way in -- or
      register the line on ONE carry status in a build that only logs, and read whether the
      damage reaction ran.

### Worth doing, and safe

- [ ] **`MotionModule::change_motion_inherit_frame` instead of `change_motion`** where the source
      uses it. It appears in 10 of the 21 ported statuses and is how the native code changes
      animation across a SITUATION change without restarting it: `SpecialSThrow_main_loop` calls
      it with `-1.0` for the frame when the whip goes ground-to-air mid-throw. This port calls
      `change_motion(..., 0.0, ...)`, which pops back to frame 0.
- [ ] **The lariat's motion rate.** `special_s_lariat_motion_rate_min` 1 and `_max` 2, applied
      with `MotionModule::set_rate`; the source scales the lariat's animation speed by how far
      away the returning victim is. Currently a `NOT MODELLED:` line.
- [ ] **`GroundModule::is_passable_check` in the carry.** Already tracked under "Still genuinely
      open" as the soft-platform carry; this is the specific call, and DK's SHOULDER_PASS is the
      status it feeds.

### Not gaps

- **`BattleObjectManager::is_active_find_battle_object`** in the rope throw. This port validates
  the victim through `crate::victim::boma`, which additionally requires the accessor to agree
  about its own id -- a stronger check, not a missing one.
- **`ArticleModule::*`** in all three up-B statuses and two whip statuses. Those are Game &
  Watch's parachute and Incineroar's rope, neither of which Pikachu has. Article work, tracked
  separately.
