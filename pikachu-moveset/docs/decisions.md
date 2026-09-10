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
