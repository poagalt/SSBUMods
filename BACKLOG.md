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

- **Sword show/hide has no precedent.** George's appendage is permanently attached and always
  visible; its plugin imports **zero `ModelModule` symbols**. Toggling mesh visibility per-move is
  the first place this project leaves the proven path.
- **Runtime scaling has no precedent either** — same finding, same cause. The Incineroar b-throw
  scaling plan is unproven.
- Appendage technique that *is* proven: extra sub-meshes under an existing mesh-object name,
  single-bound to the item-hold bones (`HaveR`/`HaveL`), new material in `model.numatb`, vanilla
  skeleton untouched.
