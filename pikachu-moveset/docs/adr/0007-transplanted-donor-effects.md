# Ported moves use their donors' own effects, through one transplanted archive

Supersedes the rule in `0002-sys-effects-only.md`. Decided 2026-10-18, at poagalt's request:
"getting all the correct visuals ... implement them exactly".

## What changed

ADR 0002 forbade fighter-owned effects because their archives are resident only when that fighter
is in the match, and the brief then forbade depending on the CSK Collection. Rat of Doom has since
become a CSK slot (c120), so the dependency ADR 0002 guarded against is already taken. And the
"no runtime escape" it measured -- no `EffectModule` call loads an archive -- has one: smashline's
`load_fighter_effects` hook (main `0x60bfd8`) loads every folder under
`effect/fighter/<fighter>/transplant/` as an extra archive for that fighter. The game's loader
(main `0x355fea0`, decompiled) picks a folder's `.eff` and `.nutexb` files by extension.

## Decision

- `tools/port-effects.py` ports each donor `effect_` script verbatim: names, modifiers, trails,
  kills, colour tints. It resolves costume branches to c00 and translates the joints Pikachu lacks
  (`feeler` -> `head`, `sword1` -> `haver` with the exact Sword1-on-HaveR transform). Anything it
  does not recognise stops the generator.
- `tools/build-effects.py` reads every donor effect name the plugin requests and merges exactly
  those emitter sets into `effect/fighter/pikachu/transplant/ratofdoom/ef_ratofdoom.eff` (12.5 MB
  for 37 effects from 14 donors, against ~150 MB for the fourteen whole archives), plus the
  sword-trail textures. `tools/effmerge` builds it: EffectConverter v1.9's own path, plus the
  texture-bank and model-bank re-saves v1.9 leaves commented out. Every texture, model and shader
  is verified byte-identical against the donor after a re-dump.
- Gate 4o still fails any effect request nothing loads; it now measures against the archive's
  manifest instead of forbidding the prefix.

## Consequences

- Moves look like their donors': Wolf's claw, Little Mac's glove lines, the swords' trails,
  Incineroar's Revenge aura and stance pose.
- The archive is loaded for every Pikachu in a match (the hook is per fighter), which costs memory
  and nothing else; vanilla Pikachu's scripts never name these effects.
