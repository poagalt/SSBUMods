# Design decisions

Running log from the design interview. Newest round last.

## Round 1

**Deliverable** — A complete mod source tree plus a `plugin.nro` verified to *compile*, handed
over with a written per-move test checklist. No in-game verification here unless the tooling
audit says it is cheap.

**Animation strategy** — Script-first. Every hitbox, frame count, damage and knockback value is
ported exactly; the moves are driven on Pikachu's *existing* animations. The character plays
correctly and looks wrong. Animation quality is explicitly a later, separate pass.

**Appendages** — Swords only. Chrom's Falchion is the single appendage asset. No ported limbs in
this pass, so the "appendage hurtbox" rule is moot for now: swords are disjoint and get no
hurtbox.

**Runtime scaling** — In scope. The list of moves that get scaled is derived from a reach audit
of all 24 slots and brought back for approval, rather than named up front. Accepted cost:
scaling Pikachu up also scales his hurtbox for the move's duration. Known case: Incineroar
b-throw, which holds the opponent overhead.

**No reflector** — Deliberate. Fox's up smash over Min Min's, with both special slots on
Incineroar, means the build ships without a reflector.

**Swords** — All four sword moves (Chrom jab, Mythra dash attack, Cloud b-air, Corrin u-air) use
Chrom's Falchion, including Cloud's b-air. Visual fidelity is a post-functionality concern.

**Costume scope** — Replaces Pikachu on all eight costume slots, `c00`–`c07`, sharing one
animation set. Vanilla Pikachu is not playable while installed. Marked "for now".

**Parity contract** — Full hitbox parameter parity: frames, damage, reach, angle, base knockback,
knockback growth, hitlag, shieldstun, SDI multiplier, autocancel and FAF windows, all copied
verbatim from the source fighter. Where a source move's length and the driving Pikachu animation
disagree, **the script wins** and the animation bends.

## Round 2

**Frame data source** — Public datamined hitbox databases (rubendal's SSBU dataset for hitbox
parameters, ultimateframedata.com for frame windows). Pulling `data.arc` off the console is held
in reserve for anything the public data underspecifies. Underspecified moves get **flagged, not
guessed**.

**Rust toolchain** — Install approved. `rustup` on the `x86_64-pc-windows-gnu` host (the existing
mingw-w64 GCC satisfies the linker; no Visual Studio), plus `cargo-skyline`, which brings its own
Switch target. No devkitPro.

**Test target** — Real hardware: a modded Switch 1 running Smash **13.0.5**, which matches
ARCropolis 4.0.9's pin. Build output goes to smashline's development-plugin hot-reload path.

**Phasing** — Three tiers. Tier 1 is the twenty conventional moves at full parity plus dodges,
attributes and Revenge; Tier 2 is G&W up-B and the get-up attack; Tier 3 is the cargo carry
state, Alolan Whip and Ptooie. Ordering only — nothing dropped.

**Attribute bundles** — Each named attribute drags in its coherent siblings, with two carve-outs:
Joker contributes fall speed *and* fast-fall speed but **not** gravity, and Little Mac contributes
run speed only (ZSS owns initial dash). Traction and midair jump count stay at Pikachu's values.

**Unassigned slots** — Vanilla Pikachu for now; tracked in `BACKLOG.md` for a later pass.

**Parity harness** — Approved. A checked-in machine-readable table of source-of-truth values per
hitbox, plus a script that diffs the emitted parameters against it.

### Gravity — delegated choice: **0.13** (Falco's value)

Reasoning:

1. **Jump height is the attribute most sensitive to gravity.** The figures you asked for — Falco's
   50.51 full hop, 17.34 short hop, 50.51 air jump, 57-frame full hop — are *computed with Falco's
   gravity of 0.13*. The stored parameter is an initial velocity, not a height. Pair Falco's
   velocity with Pikachu's 0.095 and the character floats to a far higher apex over a much longer
   arc: you would have asked for Falco's jump and received something else. 0.13 is what makes the
   number you picked actually true.
2. **It costs nothing against the fall-speed pick.** Joker's native gravity is 0.127 — a 2.4 %
   difference. One value satisfies both named attributes to within noise.
3. **It matches the weight class.** Bowser sits at 0.125; heavyweights cluster 0.12–0.13.
4. **It fits the design.** 0.13 is high. Combined with Joker's fast-fall of 3.097 — a 1.9×
   multiplier, near the top of the cast — the character accelerates downward hard and lands fast,
   which is the counter to being juggled at 135 weight. Terminal velocity stays moderate at 1.63,
   so it is not Fox-tier combo food. Jigglypuff's 1.332 air speed and 0.13 air acceleration give
   back the drift high gravity would otherwise cost during recovery, which matters because G&W's
   up-B has almost no horizontal component.
5. **Autocancels still work.** Full hop lasts 57 frames: Cloud b-air (AC 37), Corrin u-air (AC 38)
   and K. Rool d-air (AC 40) all fit. Short hop is 32 frames and Kazuya f-air (AC 25) fits.
   Palutena n-air (AC 40) will not autocancel from a short hop — same as on Palutena herself.

The choice is not delicate: 0.125 and 0.127 are within noise of 0.13.

## Round 3

**Non-hitbox properties** — In scope. Armor, intangibility and autocancel windows port alongside
hitbox parameters, sourced from ultimateframedata.com Notes plus public ACMD dumps. **Exception:
K. Rool's down air ships without belly armor** — see ADR 0001, which generalises this into a rule.

**Deployment** — No FTP on the console; files reach the SD card via the hekate GUI and homebrew
USB transfer. Priority is "better mod first, convenience second", so the **packaged ARCropolis
mod folder is the deliverable**. The smashline `development.nro` hot-reload file is emitted too,
since it is free and lets a script-only change be swapped as one small file instead of the whole
folder. cargo-skyline's FTP install/log features are unusable here.

**Parity harness** — Codegen. Move scripts are **generated from the parity table**, making them
correct by construction; the harness then re-verifies the table against the scraped source data.
Generated scripts carry a do-not-edit header; hand-tuning happens in the table.

**Tier 1 review** — One batch at the end, not category-by-category.

### Get-up attack — the brief's premise was wrong

The brief said to find the best get-up attack on ultimateframedata.com. **UFD does not publish
get-up attack frame data** — the `Getup Attacks` block on each character page is hitbox GIFs only,
with no numeric fields. Data was instead taken from the game's own 13.0.1 scripts via rubendal's
dataset and cross-validated against UFD (agreeing on 87/89 characters for the adjacent Ledge
Attack, differing by one frame on Sheik and Sora).

**Get-up attacks are almost entirely standardized**, which makes most of the question moot:

- **Total frames: 45 for all 89 fighters.** Zero variance. FAF 46.
- **Damage: 7.0%** for 86/89 face-up and 87/89 face-down.
- **All 178 get-up attacks hit both sides.** Universal.
- Intangibility ends exactly on the last active frame — 178/178, no exceptions.

The only two real levers are **intangibility end** (vulnerable tail = 45 − intangibility) and
**hitbox reach**, which is not standardized: 10 to 54.5 game units, median 38.

**Chosen: Bowser.** Reach 54.5 units — a genuine outlier against 51 for second place and a median
of 38 — with intangibility 1-23 covering every active frame (19-20 and 22-23), so the attack
cannot be beaten cleanly, only waited out. Coherent with the build already taking Bowser's weight.
Runner-up is Mythra/Pyra: frame 14 startup (tied fastest), 46.5 reach, intangible 1-25.

Adjacent finding, for the ledge attack slot: **55 total frames for all 89**, and every ledge
attack is active for exactly 3 frames ending with intangibility. Best is **Ganondorf** — `S+`
range, 10%, intangible 1-26, -21 on shield, the only fighter near the top of all four axes at
once. Bowser has the single longest range (`SS`) at the cost of 1-24 and -23.

### Confirmed source of truth

rubendal's dataset exposes raw per-character ACMD script data at
`https://raw.githubusercontent.com/rubendal/ssbu/gh-pages/data/patch/13.0.1/character/<Name>/data.json`,
carrying Damage, Angle, KBG, BKB, Size, Z/Z2 offsets, ShieldDamage, bone anchor, intangibility
windows and FAF. This is exactly the field set the parity contract requires, and it settles the
Round 2 Q1 risk.

## Build environment

Rust 1.98.1 stable + nightly on the `x86_64-pc-windows-gnu` host, `cargo-skyline 3.5.0`, and the
skyline Switch toolchain at `~/.cargo/skyline/toolchain/skyline` — the same path that appears in
George's own build fingerprints, so the build environment now matches the reference mod's.

One local fix was required: w64devkit ships a slim GCC that folds the unwinder into `libgcc.a`
and provides no separate `libgcc_eh.a`, which Rust's gnu host linker requests by name. Added an
alias copy at `w64devkit/lib/gcc/x86_64-w64-mingw32/16.1.0/libgcc_eh.a`. Additive and reversible —
delete that one file to undo.

## Round 4

**Trait rule boundary** — Terry's special-cancel is excluded as a character-wide trait, and rather
than ship a hollowed-out Terry f-tilt (−8 instead of +11 on shield), **f-tilt changes source
entirely to Mythra**. Little Mac's f-smash super armor stays as the named exception in ADR 0001.

**Dodges** — Timing only. Fox's windows and totals port; Pikachu's roll and air-dodge distances
are accepted. Since intangibility lives in `motion_list.yml` as `xlu_start`/`xlu_end` and totals
as `cancel_frame`, this needs no animation work at all.

**Parity table format** — Reversed from the earlier TOML recommendation. The table will not be
read or hand-tuned by a human, so human-friendliness buys nothing and the priority is eliminating
transcription error. The table is therefore **JSON, generated directly by scraping rubendal's
`data.json` and normalised**, never hand-authored — matching the source shape removes an entire
translation step. Codegen consumes it; the harness re-verifies the committed table against a
fresh scrape.

**Name** — Placeholder **"Rat of Doom"** across the mod folder, crate and `info.toml`. Tracked in
`BACKLOG.md` for renaming.

**Get-up vs ledge attack — confirmed distinct.** SmashWiki: *"While both are categorized as
'get-up attacks', edge attacks and floor attacks are distinct moves."* A floor/get-up attack is
performed while **lying on the ground** after a knockdown; a ledge attack while **hanging on an
edge**. There are in fact three floor-adjacent moves, all separately parameterised and all
standardized cast-wide:

| Move | Performed from | Total frames | Damage |
|---|---|---|---|
| Get-up attack (face up / face down) | lying on the ground | 45 | 7.0 % |
| Trip attack | the sitting/tripped state | 49 | 5.0 % |
| Ledge attack | hanging on an edge | 55 | 9–10 % |

Ultimate also made edge attacks *universally* intangible until their hitboxes stop being active.
Get-up attack is **Bowser**; ledge attack is **Ganondorf**; the trip attack remains unassigned.

**Appendages** — Sword visibility toggles per-move via `ModelModule::set_mesh_visibility`, with
a permanently-attached prop as the declared fallback.

**Runtime scaling** — Moved to Tier 2, off the critical path.

**Effects** — `sys_*`-only rule adopted; recorded as ADR 0002. A CSK-requiring variant with full
source effects is tracked as a separate future deliverable.

**Codegen scope** — One JSON parity table drives two generated artifacts: the Rust hitbox scripts
and the patched `motion_list.yml`.

## Round 5

**Motion rate** — Uncapped. When a source move's length and the driving Pikachu animation
disagree, `FT_MOTION_RATE` stretches or compresses to fit however far that goes, including
extreme slowdowns. Frame data is unaffected; this is purely what gets drawn, and it will be
judged on-console rather than pre-emptively clamped.

**Trip attack** — Bowser, matching the get-up attack. Same source, same reach, for what is
effectively the same defensive situation.

**Sword asset** — Open. Chrom's Falchion is a sub-mesh of his body model and cannot be isolated
without Blender, so an in-game substitute is being researched.

## Implementation findings

**The parity source changed, for the better.** `WuBoytH/SSBU-Dumped-Scripts` ships all 96
fighters' ACMD already converted to smashline Rust. `tools/gen-moves.sh` copies those scripts
**verbatim** and renames the agent, so frame-data parity holds by construction with no
transcription step at all. The JSON-table codegen agreed in Round 4 was aiming at exactly this
property; this reaches it more directly. The table survives as a verification artifact, not a
generator.

**Only `game_*` scripts are ported.** Vanilla `effect_*` scripts reference fighter-prefixed
effect archives, which ADR 0002 forbids, so Pikachu keeps his own effects.

**Four hitboxes anchored to bones Pikachu lacks** — `sword1` (Chrom jab), `stick` (Palutena
n-air), `to_open`/`to_close` (Mythra dash and f-tilt) — are remapped to `haver` by
`tools/bone-remap.sed`. Offsets are inherited from the source fighter and are **not** recalibrated
for Pikachu's proportions, so those four have approximate placement.

**A strings dump of the skeleton is not a valid bone list.** It reports `top` as absent, yet
Pikachu's own scripts use it 287 times. `tools/check-bones.sh` therefore validates against
Pikachu's own ACMD usage as well.

**Pikachu has no `attack_s4_hi` / `attack_s4_lw` motion entries**, so Little Mac's angled f-smash
variants have nowhere to run and are deliberately omitted. The straight version, with its super
armor, is what ships.

**Specials are gated on motion entries, not on scripting.** Pikachu's Quick Attack uses
`special_hi_start` / `special_air_hi1` / `special_air_hi2`; there is no plain `special_hi` for
G&W's up-B, no Incineroar side-B states, and no `throw_f_{f,b,hi,lw}` for DK's cargo throws.
The fix is that **a new motion entry needs *an* animation file, not a *retargeted* one** —
`tools/motion-patch/src/bin/synth.rs` clones an existing Pikachu entry as the template and
overwrites only `extra` and `flags` from the source fighter. Ten entries synthesized this way.

**Revenge is deliberately not shipped.** Its counter detection, three-stack storage, 60-second
timer and damage/knockback multiplier live in fighter-specific status logic and work variables,
none of which is in the ACMD dumps. Porting only the ACMD would produce a down-B that poses like
Revenge without countering — worse than leaving Thunder in place.

**Risk on the up-B:** George's status scripts reference
`FIGHTER_GAMEWATCH_INSTANCE_WORK_ID_FLAG_SPECIAL_HI`. Instance work IDs are per-fighter, so using
G&W's index on Pikachu may collide with a Pikachu variable. George ships exactly this pattern on
Diddy, so it is the proven path, but it is untested here and it replaces the recovery.
