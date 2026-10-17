# ADR 0005 — A cloned article stays faithful to its original owner; it is not rescaled

Status: accepted, 2026-09-14.

## The decision

When a move needs an article — a projectile, a rope, Piranha Plant's Ptooie ball — the builder
gives the host a **clone of the source fighter's article**, and does not adjust its scale, its
params, or its reach to suit the host. The deviation is **recorded in the output** instead.

So Piranha Plant's spikeball on Pikachu is a genuine Piranha Plant spikeball: packun's size,
packun's speed, packun's reach, on a fighter who is a fraction of packun's height.

## Why this is not a compromise

It is what the engine does, and that was established by root-causing the clone crash rather than
by reasoning about it. `clone_weapon` appends the source's article descriptor verbatim —
`weapon_id`, `on_init_callback`, `on_fini_callback` — so the engine calls
`InstantiateBattleObject(1, WEAPON_KIND_PACKUN_SPIKEBALL, host_entry)` and the object really is
the Piranha Plant class. **Its native `WeaponSpecializer_PackunSpikeball` code runs**, and that
code reads packun's own param object out of a hardcoded slot:

```
0x3476e38  ldr  x20, [x8, #0x1228]    ; entry[FIGHTER_KIND_PACKUN].param_object
0x3476e44  ldp  x8, x9, [x20, #0x138]
```

`0x70 + 0x38 × 0x51 = 0x1228`, and `FIGHTER_KIND_PACKUN = 0x51`. The article is not a model with
the host's behaviour attached; it is the source's object, running the source's compiled code
against the source's numbers. There is no seam at which a host-appropriate scale could be
introduced without fighting the engine.

## Why not rescale it anyway

The parity standard this project works to is *"as if the source character were dropped into the
host's exact circumstances in that moment"*. An article **is** the source's object, so leaving it
alone is the standard rather than a shortcut.

And rescaling would mean inventing a factor. `pikachu-moveset/docs/adr/0005` forbids a fix without
evidence, and the hitbox work only got away with scale compensation because the `scale` param is
read out of the game — two hand-copied scale constants in this project were wrong by 5% and 29%,
and the trace reported 0.00 error throughout because it applied the same wrong constant to both
sides. There is no equivalent published number for "how big should another fighter's projectile be
on this host", because the question does not arise in the game.

## Considered and rejected

**Compensate, the way `topscale` scales `top`-anchored hitbox offsets.** Superficially consistent,
actually different: `topscale` multiplies coordinates in a script the builder generates, using a
ratio read from `fighter_param.prc`. An article's geometry and behaviour live in native code and a
param object the builder does not own, so the equivalent operation does not exist — the closest
thing would be `PostureModule::set_scale` on the article, which changes the model and not the
params driving it, producing something that looks right and behaves wrong.

**Accept for now, revisit if a playtest reads wrong.** Rejected as a decision, kept as an
expectation: it *will* read wrong, because a Piranha Plant ball is large. The point of this ADR is
that "it looks too big" is not by itself grounds to change it.

## Consequences

- **This will look odd, and someone will try to fix it.** That is the whole reason the decision is
  written down. A future reader seeing a spikeball dwarf its owner should find this file before
  reaching for a scale multiplier.
- The output must name the deviation per article, in the same place it names the fighter-wide
  traits it declined to port.
- If a host genuinely cannot accommodate an article — it collides with a slot the host needs, or
  the host has no free index — that is a **refusal**, routed through the same path as the roster
  tiers in [ADR 0003](0003-declared-support-tiers.md). Not a silent rescale, and not an
  out-of-range article id, which the engine does not validate: `generate_article` indexes
  `descriptor[id]` *before* comparing it against the count and silently routes an out-of-range id
  to the **item** path instead of erroring.
