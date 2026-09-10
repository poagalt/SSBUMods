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
