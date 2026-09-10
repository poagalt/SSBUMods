# Backlog

**Authoritative.** Every outstanding item and planned future item for this repo lives here.
If it isn't in this file, it isn't tracked.

---

## pikachu-moveset

### Tier 1 — conventional moveset (current work)

- [ ] Source-of-truth hitbox table for all 20 conventional moves, from public datamined data
- [ ] Parity verification harness (static diff of emitted params against the table)
- [ ] Jab (Chrom) · Dash attack (Mythra) · F-tilt (Terry) · U-tilt (Falco) · D-tilt (R.O.B.)
- [ ] F-smash (Little Mac, **super armor f8–15 / f9–15 down-angled**) · U-smash (Fox) ·
      D-smash (Game & Watch)
- [ ] N-air (Palutena) · F-air (Kazuya) · B-air (Cloud) · U-air (Corrin) · D-air (K. Rool)
- [ ] Grab (Palutena) · Pummel (Kazuya) · B-throw (Incineroar) · U-throw (Mewtwo);
      D-throw stays Pikachu
- [ ] Down B — Revenge (Incineroar): stored multiplier, 3 stacks, 60 s timer, lost on grab or 36 %
- [ ] Spot dodge, forward roll, back roll, air dodges (Fox)
- [ ] Attributes: weight (Bowser) · initial dash (ZSS) · run speed (Little Mac) · walk (Marth) ·
      air speed + air accel (Jigglypuff) · jump heights (Falco) · fall + fast-fall (Joker) ·
      **gravity 0.13** (chosen — see `docs/decisions.md`)
- [ ] Reach audit across all 24 slots → proposed runtime-scaling list for approval
- [ ] Chrom Falchion appendage on the four sword moves (jab, dash attack, b-air, u-air)

### Tier 2

- [ ] Up B — Game & Watch Fire, including the parachute state
- [ ] Get-up attack (source under research)
- [ ] Ledge attack (recommendation to be folded into the get-up attack research)

### Tier 3 — new state machines, each potentially a session on its own

- [ ] F-throw — DK cargo carry state: carry walk, carry jump, carry turn, plus all four
      cargo throws
- [ ] Side B — Incineroar Alolan Whip: command grab with three branching follow-ups
      (Lariat / Back Body Drop / Chest Bounce) and damage-based armor
- [ ] Neutral B — Piranha Plant Ptooie: an article with its own model, hittable by both players

### Deferred — left vanilla Pikachu for now, to be changed later

- [ ] Ledge attack visuals *(gameplay side is Tier 2)*
- [ ] Taunts (up / side / down)
- [ ] Victory animations and victory theme
- [ ] Voice clips and SFX
- [ ] CSS name, portraits, stock icons, boxing-ring title
- [ ] **Animation pass** — the whole visual layer. Tier 1 ships driving Pikachu's existing
      animations, which will look wrong by design. Retargeting real source animations onto
      Pikachu's skeleton is a separate project requiring Blender and the Smash Blender addon,
      neither of which is installed.
- [ ] Per-source-fighter appendages beyond the sword (Mac's glove, K. Rool's foot, Kazuya's leg)
      and their hurtboxes

### Open edge cases — decide before they bite

- [ ] Kirby's copy ability: neutral B becomes Ptooie, an article. Smashline exposes
      `smashline_whitelist_kirby_copy_article` — decide what Kirby inherits.
- [ ] Pikachu's own articles (Thunder Jolt, Thunder) are orphaned once neutral B and down B are
      replaced. Decide whether to strip them.
- [ ] Costume scope is "all eight slots, for now" — revisit if vanilla Pikachu is wanted
      alongside.
- [ ] No reflector in the build (deliberate). Revisit only if the character plays badly against
      projectiles.

### Known risks

- **No source frame data on this machine.** No `data.arc`, no romfs, no game. Tier 1 depends
  entirely on public datamined databases being complete enough. Any move they underspecify gets
  flagged rather than guessed.
- **ARCropolis 4.0.9 is pinned to Smash 13.0.5.** A console update breaks the whole toolchain.
- **No local test path.** Nothing can be verified in-game here; hardware is the only target.

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
