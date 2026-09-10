# Character-wide traits do not port with a move

The parity contract says a ported move copies its source's frame data, damage, reach and
non-hitbox properties verbatim. But some of those properties belong to the source *fighter*
rather than to the move: K. Rool's belly armor appears across most of his moveset because he
has a belly, not because his down air is special. Porting it would smuggle a K. Rool passive
onto a character that has no belly, and the same logic would drag in Terry's special-cancel
system, Cloud's Limit gauge and Mythra's Foresight.

**Decision:** properties specific to a single move port; properties the source fighter carries
across their moveset do not.

## Consequences

- K. Rool's down air ships **without** belly armor.
- Little Mac's forward smash super armor is an **explicit exception** — it is arguably a Mac-wide
  trait (all his smashes have it), but the brief names it directly, so it ports.
- Terry's "special cancellable" property is a Terry system mechanic and is expected to be
  excluded, pending confirmation.
- Move-specific intangibility windows (Terry's f-tilt leg intangibility, Fox's u-smash) still
  port — these are per-move script data across the whole cast, not passives.
