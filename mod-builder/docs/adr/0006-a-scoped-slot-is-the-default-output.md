# ADR 0006 — A generated mod adds a costume slot; it does not replace the host

Status: accepted, 2026-09-14.

## The decision

The builder's default output gives the custom character **its own costume slot** on a host fighter
— the host's vanilla slots keep working, and the player picks the custom character by picking that
slot. Replacing a roster member agent-wide stays available as an opt-in.

## Why this is worth recording

**Every moveset mod this project learned from does the opposite.** `pikachu-moveset` installs
agent-wide on `pikachu`, so all eight costumes become the custom character and vanilla Pikachu is
gone while the mod is enabled. So does George, on Diddy. A future reader comparing the builder's
output to the reference implementations will find it doing something different and want to know
why.

The reason is that the audience changed ([ADR 0001](0001-a-tool-anyone-can-use.md)). A tool of one
can happily delete Pikachu; a tool that hands someone a mod which deletes Terry to give them
Waluigi is worse than one that adds Waluigi to Terry's slot 120.

## The mechanism, and that it needs no new capability

Established by reading four shipped slotted characters (Waluigi on `dolly` c120–127, Thwomp on
`ike` c40–47, Herobrine on `mewtwo` c32–39, Gogeta on `dolly` c103–110). All four are node-for-node
identical:

- **5 `new-dir-infos` per slot**: `fighter/<k>/cN`, `camera/cN`, `movie/cN`, `result/cN`,
  `kirbycopy/cN`. `movie` and `result` ship **zero files** — they exist so a lookup cannot fail.
- **5 `new-dir-infos-base` per slot**, each cloning the `c00` equivalent: `cN/cmn`, `cN/camera`,
  `kirbycopy/cN/{cmn,sound,bodymotion}`.
- Code is scoped with `smashline::Agent::set_costume(vec![N])`. The plain installers are
  five-instruction thunks that tail-call the `_costume` ones with a null slice, so an empty
  costume list literally means "all costumes".
- A zero-byte `<name>.marker` in `fighter/<k>/model/body/cN/` tells the plugin which slot is its
  own. Only existence is tested — one shipped mod's marker is 324 bytes of accidentally-pasted
  AutoHotkey and works fine.

Slot numbers are convention, not structure: c120, c40, c32 and c103 all work, and c103 is not even
aligned to a multiple of eight.

## The revision, recorded because the reasoning matters more than the answer

**This decision was first taken as a per-project mode with a refusal, and revised within the hour.**
The original reasoning was that `fighter_param.prc` has no costume dimension, so a slotted
character could not have its own weight, speeds, gravity or jumps — which would have made a scoped
slot cost half of what the builder does, and argued for replacement as the default.

The premise was false. `fighter_param.prc` genuinely has no costume dimension, and **nobody uses
it for this**: `lib_paramconfig` hooks the engine's param getter, reads
`FIGHTER_INSTANCE_WORK_ID_INT_COLOR` (`0x10000042`, verified against the constant table), and keys
its override table on **(fighter kind, costume slot, param, subparam)**. A slotted character can
have its own attributes.

So the refusal that the mode existed to carry had nothing to refuse, and the safer default became
the better one. The lesson is the one ADR 0005 of the moveset context already states: a constraint
inferred from a file format is not a constraint on the system.

## Consequences

- **A fifth-party dependency is now load-bearing**: ParamConfig (`libparam_config.nro`), for
  attributes to be slot-scoped at all. It joins ARCropolis, smashline, and CSK.
- **`vl.prc` still does not scope**, and no shipped mod solves it. A cloned article's params apply
  to every costume of the host including the vanilla ones. That is a real fidelity leak in scoped
  mode and it must be reported rather than hidden.
- **One trap to generate around:** `ACMD_SCRIPTS` is keyed by `(agent, costume_set)` with a single
  map lookup, while status scripts and callbacks are filtered per item. So mixing agent-wide and
  scoped ACMD on one agent **silently drops the agent-wide scripts** for that costume. The
  generated rule is: if any ACMD on an agent is scoped, scope all of them.
- Wildcard agents (`"fighter"`, `"weapon"`) cannot be scoped, and overlapping costume groups are
  undefined — the source has a literal `TODO` where the collision would be handled.
