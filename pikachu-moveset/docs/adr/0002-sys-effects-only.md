# Only `sys_*` visual effects may be used

The "must not require the CSK Collection" constraint is enforced by two mechanisms, and this is
one of them. Fighter-prefixed effect archives are not resident unless that fighter is loaded, so
referencing one from Pikachu's scripts is precisely what creates a CSK dependency. The reference
mod uses 22 distinct effects, every one from the always-resident `sys_*` archive, and zero
fighter-prefixed names anywhere in its binary.

**Decision:** ported moves may only use effects from the universal `sys_*` archive (and the
always-loaded item archive). A ported move does not get its source fighter's signature visuals.

## Consequences

- No Cloud sword trail, no Little Mac punch flash, no Mythra afterimages. Ported moves get
  generic smoke, arcs, impacts and flashes.
- This will look wrong, and a future reader will be tempted to "fix" it by wiring up the real
  effect names. Doing so silently reintroduces the CSK dependency the brief forbids.
- A CSK-requiring variant with full source effects is tracked in `BACKLOG.md` as a separate
  deliverable, not as a fix to this one.
