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
