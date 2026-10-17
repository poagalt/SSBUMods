# ADR 0008 — Source-faithful effects are a priced option, and they are not a CSK feature

Status: accepted, 2026-09-14.

## The decision

The default output uses only the always-resident `sys_*` effect archive. Source-faithful effects
are an **opt-in build** that depends on the **One-Slot Effects** plugin and costs a rebuilt host
effect archive **per slot**, measured at ~21 MB each.

It is **not** called a "CSK build". That name was wrong and is retired.

## Why the name mattered enough to record

This plan called the option a "CSK build" for most of a day, on the reasonable assumption that
The CSK Collection — which everything else about slotted characters depends on — was also what
made foreign effects work.

**It is not.** An exhaustive search of all 1,425,408 bytes of `libthe_csk_collection.nro` for
`effect`, `.eff`, `ef_`, `EffectModule` and related tokens returns **one** hit:
`effect_load_group_hash`, a field of the *stage* redirection table. CSK's "Enable One Slot Series"
flag is about UI series icons — its neighbours in the binary are
`set_chara_db_ui_series_id_lookup` and `results_screen_set_series`. The 5.0.6 package ships one
NRO and it is not `libone_slot_eff.nro`.

A future reader who sees "CSK build" will go looking for effect support inside CSK and find
nothing. That is a day lost to a wrong signpost, which is the cheapest kind of mistake to prevent.

## What the option actually is, and what it costs

The shipped approach is not "make the source's effect archive resident". It is **merge the
source's effects into the host's own archive**:

> Gogeta ships eight files `effect/fighter/dolly/ef_dolly_c103.eff` … `ef_dolly_c110.eff`, each
> **21,672,480 bytes — 173 MB total.** Each is the host fighter's effect archive rebuilt with the
> imported effects added. One-Slot Effects selects which `.eff` a given slot loads.

So the archive still belongs to the host, which is why it is resident — and that is exactly the
reasoning `pikachu-moveset/docs/adr/0002` gives for the `sys_*` restriction. **That ADR's premise
survives this investigation intact.** Fighter-prefixed archives are not resident unless that
fighter is loaded; nothing in the ecosystem defeats that, and the workaround is to stop the archive
being foreign.

## Considered and rejected

**Drop the option entirely.** Tempting at 21 MB per slot plus a fifth plugin dependency. Rejected
because fidelity is the project's stated goal and a move without its signature visuals is visibly
not the move — this is the one axis where `sys_*` is a compromise rather than a correct answer.

**Reference the source's archive instead of merging.** This is what "CSK build" implied and it is
not available. See above.

## Consequences

- Choosing the option pulls in **One-Slot Effects** as a dependency, and per
  [ADR 0006](0006-a-scoped-slot-is-the-default-output.md)'s note about dependency guards, the
  generated mod must check for it **only when the option is used**.
- The size cost scales with slots, not with moves: a one-slot character pays ~21 MB, an eight-slot
  character pays ~173 MB. That belongs in the builder's output summary, because it is the kind of
  number a user should see before it appears in their SD card.
- Two of the four surveyed mods take this option, so it is the normal price rather than an exotic
  choice.
