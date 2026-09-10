# ADR 0003: Pikachu plays the source fighter's own animation

Status: accepted
Date: 2026-09-10
Supersedes: the hitbox re-anchoring approach (`tools/reach/src/bin/reanchor.rs`) and the
synthesised f-smash lunge (`tools/lunge.sh`).

## Context

Round 1 settled on a script-first architecture: port each move's ACMD verbatim and leave Pikachu
playing his own animation. Hitbox offsets in SSBU are **bone-local**, so this broke every move whose
source hitboxes ride a limb: on the source fighter the animation carries the bone into position,
while Pikachu's animation leaves it near his body. Observed in game as Falco's u-tilt second hit
never connecting and Fox's u-smash "seeming too small".

`reanchor.rs` compensated by computing each hitbox's world position under the *source's* animation
via forward kinematics and re-expressing it relative to `top`. Three problems, in increasing order
of severity:

1. **A hitbox anchored to `top` is frozen relative to the fighter.** A real bone-anchored hitbox
   sweeps as the bone sweeps. Re-anchoring samples the arc at one frame and pins it there, so a
   sweeping hit becomes a static one at an arbitrary point along the sweep.
2. **Some of those sampled points are unhittable.** A hitbox only connects if it intersects the
   x≈0 plane every fighter stands on, i.e. `|x| <= size`. The vanilla game honours this in 98% of
   the 1,660 top-anchored hitboxes that use non-zero depth (Kazuya's `x=-10.0` ships with
   `size=10.0`). Ours violated it on **14 of 53** capsule far ends, max `|x2|` = 16.69 against a
   size of ~3.5. Those ends are dead geometry in the background.
3. **It cannot reproduce root motion at all.** Little Mac's f-smash range is a 21.45-unit forward
   lunge in his animation's `Trans` track. `lunge.sh` synthesised it with `SET_SPEED_EX`, an
   uncalibrated guess fighting ground friction.

The user's clarifying requirement rules the compensation approach out on principle:

> all moves should have hitboxes accurate to if the character that normally uses them were dropped
> into our modified pikachu's exact circumstances in that moment

## Decision

Ship the source fighter's own `.nuanmb` in Pikachu's motion directory, and port each ACMD
**verbatim including its original bone anchors**. Every hitbox is then correct by construction,
because the bone it rides is doing exactly what it does on the source fighter.

Four measurements establish that this works:

- **Filenames are universal per motion.** `attack_s3_s` is `c01attacks3s.nuanmb` on *every*
  fighter. Verified for all 11 ported attack motions, source vs Pikachu: 11/11 identical. A
  drop-in is therefore a same-path file replacement needing no `motion_list` or `config.json` change.
- **Every bone our hitboxes anchor to exists in Pikachu's skeleton** — all 12 of `armr`, `arml`,
  `kneer`, `kneel`, `toel`, `toer`, `shoulderl`, `shoulderr`, `handr`, `handl`, `haver`, `havel`.
  Overlap with the sources runs 37-66 bones. Tracks naming bones Pikachu lacks are ignored, which
  is what lets one animation file drop into many fighters.
- **The animation carries the source's skeleton.** Limb bones animate rotation only; their
  translation is constant and **equal to the source's bind translation** (Mythra `ArmR` 2.421 in
  both her skeleton and her f-tilt animation, against Pikachu's 1.302). Playing her animation
  therefore gives Pikachu her bone lengths for that move's duration, which is why the hitboxes
  land where hers do — and, as a free consequence, why his limbs visibly extend to meet them.
  That retires the separate "appendages" work item for these moves.
- **Our patched `motion_list` is already primed for it.** We copy the source's flags wholesale, so
  `attack_s4_s` already carries `MOVE` where vanilla Pikachu does not, and `attack_hi3` already
  clears the `MOVE` Pikachu has but Falco does not. Pre-flight across all 11 motions: 0 warnings.

## Consequences

- `reanchor.rs` and `lunge.sh` are removed from the build. Keeping the lunge would now
  **double-count**: `Trans` supplies the movement natively once Mac's animation is in place.
- Copying source flags — which an outside review recommended stopping — is *correct* under this
  architecture and must be kept. The review's advice was sound for the script-only design and is
  wrong for this one.
- Root motion, animation length, hitbox sweep, hitbox depth and limb reach are all inherited
  rather than synthesised. Five approximations become zero.
- Pikachu deforms toward the source's proportions during these moves. Accepted: the user chose
  full parity, with visual work deferred.
- **Residual inexactness:** the game scales a fighter's skeleton by its `scale` param, and
  Pikachu's is 0.96 against e.g. Mythra's 1.112. A raw drop-in therefore lands at
  `0.96/1.112` = 86% of her world reach. Correctable exactly by pre-multiplying the animation's
  translation tracks by `source_scale/0.96`; deferred to a follow-up so the raw drop-in can be
  validated on its own first. Ratios range 1.00 (Chrom) to 1.16 (Mythra).
- Moves whose animation is not yet extracted (d-tilt, d-smash, throws, defensive, dodges, and the
  angled f-tilt/f-smash variants) keep Pikachu's animation for now and remain subject to the old
  mismatch. Tracked in the backlog.

## Addendum: the body lift is accepted, 2026-09-10

A dropped-in animation supplies the source's `Rot` standing height in place of Pikachu's 5.22
model units, so his body is drawn 5.4 to 6.0 world units higher during these moves:

| aerial | source standing height (world) | Pikachu's own | lift |
|---|---|---|---|
| n-air (Palutena) | 10.45 | 5.01 | +5.44 |
| f-air (Kazuya)   | 10.71 | 5.01 | +5.70 |
| b-air (Cloud)    | 11.02 | 5.01 | +6.00 |
| u-air (Corrin)   | 10.89 | 5.01 | +5.88 |
| d-air (K. Rool)  | 10.51 | 5.01 | +5.50 |

Playtest read this as the aerials "carrying pikachu higher/lower depending on the jump height of
the character the attack came from". It is not movement: all ten aerial and landing motions have
the `move` flag clear and their `Trans` vertical tracks are flat (range 0.00-0.01), so his
trajectory is his own physics. The variation between sources is only 0.56 world units.

The lift is load-bearing. It is the same thing that makes his visible limb reach the hitbox, and
grounded moves need it for his feet to stay on the ground, because the leg chain scales with it.
Removing it would drag the hitboxes down by the same amount -- they would stay matched to his
limbs but would no longer be at the source's absolute height, cutting vertical reach on aerials.

Decision (user, 2026-09-10): **keep source-exact.** Accepted consequence: during these moves he is
drawn taller and his bone-anchored hurtboxes ride up with him, so he is a bigger target than at
rest. Two alternatives were offered and declined -- anchoring his body at his own height
everywhere, and anchoring only the five aerials.
