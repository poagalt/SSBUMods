# ADR 0003 — Declared support tiers per fighter and per role, not universal support

Status: accepted, 2026-09-14.

## The decision

"Any fighter's move onto any other fighter" is not offered universally. Each fighter carries a
declared tier — **supported**, **supported with caveats**, or **refused with a reason** — and the
tier differs by **role**, because a fighter can be a fine source and an impossible host.

The builder validates the triple against those tiers before doing any work, and a refusal is a
normal output rather than a failure.

## Why: the roster does not honour the obvious model

A tool naturally assumes *one fighter = one agent = one skeleton = one status enumeration*. Twelve
identifiable cases in the roster break that, measured from the local corpus:

- **The three Mii Fighters have no `model/body` at all.** They carry 45–56 per-outfit model
  directories, each with its own `model.nusktb` and each with twelve colour variants. Bone
  grafting and animation drop-in both need one body skeleton to target, and there is no single
  skeleton here to target. They can source a move; they cannot receive one. (Those twelve variants
  are outfit *colours*, not roster costume slots — an earlier draft of the builder's plan read them
  as evidence that a fighter can have more than eight slots, and that was wrong.)
- **Kirby** declares **412** fighter-specific status kinds, **375** of them copy-ability statuses
  parameterised by 89 *other* fighters, plus 111 `copy_*` article directories and 89 extra motion
  lists. His own 37 statuses port out fine; hosting on him means allocating past `0x37D` and
  reasoning about a copy system.
- **Pokémon Trainer and Aegis are director agents.** `fighter/ptrainer/` has no `model/body`, and
  `lua2cpp_ptrainer.nro` is not in the decomp corpus at all. `element` is stranger still: it owns
  six status kinds and its NRO contains **no `element/element/` fighter agent** — only the weapon
  agents `changer` and `diver`.
- **Twelve echo fighters declare zero own status kinds**, reusing the original's enumeration —
  including **Chrom**, a source this project already ships a move from. The clone→original mapping
  could not be proven from the corpus, only inferred from naming.
- **Four fighters terminate their enumeration with `_MAX`, not `_NUM`** — `PEACH` 0x1EA, `KOOPA`
  0x1EB, `PIT` 0x1EE, and **`DONKEY` 0x202**. There is no `FIGHTER_DONKEY_STATUS_KIND_NUM`. A tool
  keying on `_NUM` silently finds nothing for the cargo-throw source.
- **`FIGHTER_GAOGAEN_REVENGE_STATUS_KIND_*` is a second, zero-based, 17-entry enumeration** inside
  the Revenge source's own namespace. A glob on `FIGHTER_<NAME>_STATUS_KIND_*` mis-assigns all 17.

Against that, one simplification is worth recording because it removes a feared problem: all **91**
fighters with a `model/body` have exactly 8 costume directories with byte-identical structural file
sets. Per-costume skeleton divergence is a non-issue outside the Miis.

## The tiers as decided

**Refused as host:** the three Mii Fighters, `ptrainer`, `ptrainer_low`, `element`, `common`.

**Host with caveats:** Kirby (new ids past `0x37D`), the three Pokémon and both Aegis bodies
(their `0x1E1`/`0x1E2` swap statuses must not be disturbed), Ice Climbers (partner object), and
`koopag` (whose name is simultaneously a fighter agent and a weapon agent inside another NRO).

**Source with caveats:** the twelve echo fighters — usable for class 1 and 2, which need no status
kinds at all, and **refused for class 3** until their enumeration mapping is verified rather than
inferred. That split is what makes Chrom's jab legal today and Chrom's specials not.

## Considered and rejected

**Universal support.** Handling Kirby's copy system and the Miis' per-outfit skeletons is a
research project each, for two fighters, before the tool works for the other 93.

**Any source, restricted hosts.** Close to the decision, but it throws away the asymmetry that
makes tiering cheap: most caveats are host-side, and a blanket host restriction would exclude
Kirby, who is merely awkward, alongside the Miis, who are impossible.

## Consequences

- The refusal message is part of the product, and it must say *why* — otherwise a future reader
  "fixes" the Mii exclusion without knowing what it is protecting.
- Reading both `_NUM` and `_MAX` is not an edge case to remember; it is a requirement of the
  enumeration reader, and Donkey Kong is the proof.
- A status-kind reader must be **anchored**, not a glob. `gen-statusnames.sh` in the moveset
  context already anchors on `^FIGHTER_STATUS_KIND_` for exactly this reason, and its header says
  so; the builder inherits that discipline rather than rediscovering it.
