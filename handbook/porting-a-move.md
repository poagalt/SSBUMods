# Porting a move

Three classes of move, with sharply different costs. Establish which class you are in **before**
writing anything, because the expensive mistake in this project's history was treating a class-3
move as class 1 and patching the result for seven rounds.

| Class | Test | Cost |
|---|---|---|
| **1. Script-only** | the source status directory does not exist — the move is entirely ACMD on a stock status | hours; mechanical |
| **2. Script plus motion** | as class 1, but the host has no matching motion, or the source's FAF/intangibility differ | hours, plus an animation drop-in |
| **3. Status machine** | the source has its own `status/<Status>/` directory, or the move captures another fighter | days; a transcription, not an implementation |

The test is one `ls`:

```bash
ls vendor/ssbu-status-all/ghidra_scripts/lua2cpp_<source>.nro/<source>/<source>/status/
```

Of the 52 rows in `tools/moves.tsv`, **12 have a source status directory and 40 do not.** All
twelve are the ones implemented as status modules. That ratio is the shape of the whole problem:
most moves are cheap, and the handful that are not consumed most of the calendar.

---

## Class 1 — script-only

1. **Copy the source's ACMD verbatim.** `tools/gen-moves.sh` does this from the extracted vanilla
   scripts, emitting `@generated` modules. Never hand-edit them; the build regenerates over the
   edit and the commit contains nothing.
2. **Strip foreign constants.** A source fighter's namespaced constants must never reach the host:
   article ids and instance work ids are per-fighter enumerations, so
   `FIGHTER_ELIGHT_GENERATE_ARTICLE_ESWORD` on Pikachu indexes something else entirely.
   `tools/reach/src/bin/deforeign.rs` removes statements that reference them, with a
   `KEEP_STATUS_IDS` allowlist for the status-scoped ids a port legitimately owns.
3. **Generate a `sys_`-only effect script.** ADR 0002: fighter-prefixed effect archives are not
   resident unless that fighter is loaded, so referencing one is what creates a CSK dependency.
   `tools/extract-effects.awk` keeps the `sys_*` calls and drops the rest.
4. **Stub the host's own scripts for the slot — all four categories.** `game_`, `effect_`,
   `sound_`, and `expression_`. Stubbing only `game_` leaves the host's landing VFX and SFX firing,
   which reads in a playtest as "the landing hit is still there".
5. **Scale `top`-anchored offsets** for the size difference, if the move has any
   (`tools/reach/src/bin/topscale.rs`). It is **not idempotent** — it multiplies in place — so it
   is safe only because `gen-moves.sh` regenerates its inputs first. It refuses any file without
   the `@generated` banner for that reason.

## Class 2 — the animation

ADR 0003: **Pikachu plays the source fighter's own animation.** The alternative — keeping the
host's animation and re-anchoring the hitboxes into `top` space — fails three ways, and all three
were observed in playtests: a re-anchored hitbox is frozen where a real bone-anchored one sweeps;
some sampled points violate `|x| <= size` and are dead geometry in the background; and root motion
cannot be reproduced at all, because a 21-unit forward lunge lives in the animation's `Trans`
track, not in any script.

A raw cross-fighter `.nuanmb` drop-in works and needs no Blender. Proof: "Everyone with ganon's
down air!" ships one byte-identical 58,048-byte file into 88 different fighters' motion
directories under each host's own filename. Ganondorf's d-air has 140 tracks, Pikachu has 77
bones, and the overlap is 32 — **including `Trans`**, the root-motion track. So a drop-in gives
the source's root motion, the source's animation length, and the host's own limb lengths in the
source's pose, which is the right answer rather than a compromise.

What it costs: the host-only bones get no track and stay in bind pose (Pikachu's ears, tail, hat,
face rig, and `H_*` helpers). Ugly, not broken.

- `tools/anim-dropin.sh` ships them, trimming foreign bone and mesh tracks — untrimmed, 48 of 77
  animations resolved because the set was 38% over the memory budget.
- `tools/motion-patch` patches `cancel_frame` and `xlu_*`; `synth.rs` creates entries the host
  lacks. **Patch only those fields.** `d.flags = s.flags` imports fourteen bits from a fighter
  whose animation you may not even be using.
- Where the host has an angled variant the source does not (`attack_s3_hi`/`_lw` against Mythra,
  who has one f-tilt), repoint the entry at the animation the source does have rather than leaving
  the host's.
- `tools/motion-patch/src/bin/variants.rs` audits this **in both directions**: motions the host has
  that the source lacks, and motions the source has in a ported family with no entry of ours. The
  inverse direction is the one that catches a missing `special_air_s_*`.

## Class 3 — the status machine

ADR 0006: **transcribe the source's status tree; do not reimplement it.** The procedure:

1. **List the status's lines** (`ls` the decomp directory). Register every one the source has.
   Init, Exec, and Exit are not optional decoration; `tools/check-statuslines.sh` enforces this.
2. **Read each line's decompiled C and transcribe it**, quoting the source address in a comment so
   the diff against the source stays checkable. Where Ghidra leaves a `FUN_*` helper, read it with
   `tools/disasm --trace`, which drops the L2CValue marshalling traffic and prints 45 lines of
   engine calls instead of 400 of noise.
3. **Copy the arguments, not their effect.** `init_settings` and `set_fighter_status_data` carry
   status attributes the engine reads — `FIGHTER_STATUS_ATTR_SCALE_KINETIC_ENERGY`,
   `FS_SUCCEEDS_KEEP_ATTACK_ABSOLUTE` — and a missing one changes engine behaviour with nothing in
   the port to point at. These are common enumerations, not source-namespaced, so copying them is
   copying an argument rather than emulating an effect.
4. **Register new status kinds past the host's `NUM`**, never reuse the host's.
5. **Route through the engine wherever the engine offers a route.** The clearest example: writing
   `RAT_STATUS_KIND_SHOULDER_START` into `global_table[0x45]` makes the engine's own
   `CatchCont` send the victim into the carry, which replaced an entire hand-written interception
   of the host's forward throw.
6. **Substitute only where you must, and name it.** `whip/catch.rs` is the one substitution in the
   Alolan Whip: it tries the engine's own caught-victim first and only falls back to its own range
   test. `pikachu-moveset/docs/what-the-grabs-need.md` classifies all ~40 rows as native,
   transcribed, substituted, or intercepted, which is what makes "how native is this" a question
   with an answer.

### Transcription is necessary but not sufficient

Three failure modes survive a correct transcription, all observed:

- **Same-frame override.** `init_settings` declares `FIGHTER_KINETIC_TYPE_UNIQ` and the status's
  own `main` overrides it to `GROUND_STOP`/`FALL` on the same frame. A transcription of the first
  call alone is "correct and still wrong".
- **Invisible calls.** `sv_kinetic_energy::*` are lua-stack calls — `clear_lua_stack`, a
  `push_lua_stack` per argument, then a call taking only `lua_state_agent`. Function hooks cannot
  see them, so a runtime recording will not show them and only the disassembly will.
- **Data the host does not have.** A transcribed `WorkModule::get_param_float` is only right if
  the object it names exists on the host. See the param provider in
  [the-engine-model.md](the-engine-model.md#layer-4--params).

## Attributes, stats, and dodges

- Movement and weight are `fighter_param.prcxml` keys on the host's row: `weight`,
  `walk_speed_max`, `dash_speed`, `run_speed_max`, `run_accel_mul`, `ground_brake`,
  `air_speed_x_stable`, `air_speed_y_stable`, `air_accel_y` (gravity), `air_brake_y`,
  `air_accel_x_mul`/`_add`, `jump_y`, `mini_jump_y`, `jump_aerial_y`, `jump_initial_y`,
  `jump_speed_x_mul`.
- **Aerial landing lag is `landing_attack_air_frame_{n,f,b,hi,lw}`** — a param, one per ported
  aerial. Not a motion entry.
- **Dodge timing is `fighter_param_motion.prcxml`**, not `motion_list`: `escape_n_cancel_frame`,
  `escape_f_cancel_frame`, `escape_air_cancel_frame`, `escape_air_slide_speed`,
  `escape_air_slide_distance`. A dodge "port" that only patches motion entries is inert.
- Grounded roll distance *is* animation root motion, so `escape: flags: move: false` is how you
  kill it.
- If you change `air_accel_y` or `air_speed_y_stable`, the air-dodge FAF is computed from them
  plus `escape_air_slide_speed` — HDR has a CI job whose entire purpose is reminding you of this.

## Appendages and weapons

Two mechanisms, and they are not interchangeable:

- **Merged mesh** — single-bind geometry into the host's body model (`parent_bone_name = "HaveR"`,
  `bone_influences = []`, vertices pre-transformed into the bone's local space), then toggle with
  `ModelModule::set_mesh_visibility`. This is what vanilla itself does for the Fire Emblem swords,
  and the worked example is workshop episode 39 (Chrom's Falchion on Little Mac). **The mesh object
  needs a distinct name** — George reused an existing name with new sub-indices, which is exactly
  why George cannot toggle visibility.
- **Separate article** — `smashline::clone_weapon`, then
  `LinkModule::set_model_constraint_pos_ort(article_boma, *LINK_NO_ARTICLE, Hash40::new("have"),
  Hash40::new("haver"), CONSTRAINT_FLAG_POSITION | _ORIENTATION, false)`. `ModelModule` has **no**
  parenting call; the convention is that an article's own skeleton carries a `have` (sometimes
  `attach`) bone which is mated to the fighter's `HaveR`/`HaveL`.

`ArticleModule::have(boma, article_id, Hash40::new("haver"), …)` is the fighter-side convenience
wrapper over the same primitive. Pikachu's skeleton has `Have`, `HaveL`, `HaveR`, `HandL`,
`HandR`, `ShoulderL`, and `ShoulderR`, so every attachment point transfers. Note Pikachu also has
a `model.adjb` (168 KB) — only Chrom, Cloud, and Pikachu do — which has to stay consistent after
any mesh merge.

**The Falchion trick, worth reusing.** Chrom's sword article has skeleton `HaveR → Sword`, where
`Sword`'s local transform is the **transpose** of `HaveR`'s rotation — so the blade sits at
identity in article space. Instead of rebasing 444 vertices, append a `Sword` bone under the
host's `HaveR` carrying that same transform and leave the geometry untouched. And `Sword` (the
mesh bone) and `Sword1` (the hitbox bone) have **byte-identical bind frames**, so copying
`Sword1`'s animation track onto `Sword` lands the blade exactly where Chrom's is on every frame.

A grafted bone breaks mirroring: a hitbox anchored to a bone the host's skeleton does not really
have is not flipped by the engine, so the move works in one facing only. `tools/reach/src/bin/
rebase.rs` moves those hitboxes onto `haver`, rotating the offset by the grafted bone's transform
read from the shipped animation at the hitbox's creation frame. It fixed Chrom's jab exactly
(0.00 units) and cannot fix Palutena's n-air, because she animates `Stick` with 460 keyframes of
changing rotation and translation across a 19-frame hitbox — no static anchor follows that. That
one is deferred pending real weapon objects.

## Build order matters

`tools/build.sh` sequences the codegen because several passes consume each other's output:

```
0   rebuild the codegen tools          (a prebuilt binary silently no-ops a source change)
0a  gen-scales.sh    0b gen-motion-params.sh   0c gen-name.sh
1   gen-moves.sh     1a catch-remap    1b deforeign    1c sysfx gate
2   add-bones.sh     3 anim-dropin.sh  3b motion-patch + synth
3g  gen-rebase-anims.sh   3h reach rebase
4   topscale         5 dropin pre-flight
6   cargo skyline build --release
```

Then `tools/check.sh` (about four minutes), `tools/check-callparity.sh` (about a minute), and
`tools/deploy.sh` — **each on its own, never chained, always backgrounded**. `deploy.sh:84` says
so in as many words, and this project has paid for ignoring it twice, once with a six-hour hung
task and once with a stale lock that silently skipped the revert of a crashing build.
