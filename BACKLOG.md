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
