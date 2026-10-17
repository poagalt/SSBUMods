# The builder — language

The vocabulary of the tool, not of any one mod it produces. See [../CONTEXT-MAP.md](../CONTEXT-MAP.md)
for why this file and the root `CONTEXT.md` disagree in two places on purpose.

Only terms that have actually been settled appear here. Proposals live in `PLAN.md` until they are
decided.

## The things being named

**Host fighter**:
The vanilla fighter a move is being put onto. **A parameter, not a particular fighter** — this is
the one term the builder redefines, because the moveset context fixes it to Pikachu.
_Avoid_: base character, target, recipient

**Source fighter**:
The vanilla fighter a move or attribute is taken from. Per move, not per mod.
_Avoid_: donor, original

**Slot**:
One entry in a fighter's movelist — jab, f-tilt, up-B, pummel. The unit the builder is asked for.
_Avoid_: move, input

**Triple**:
The builder's whole input for one port: *(source fighter, slot, host fighter)*. An attribute port
is the same shape with an attribute in place of the slot.
_Avoid_: job, request, spec

## What the builder knows

**Game Database**:
The queryable store of everything the builder reads out of the game — statuses and the lines each
one has, motions, scripts, params, skeletons, articles, constants, entry points. Derived, versioned
by game version, and **built on the operator's machine rather than shipped**.
_Avoid_: cache, index, dump

**Requirements manifest**:
The computed, per-triple record of everything a move needs: its scripts, motions, params, statuses,
foreign constants, bones, articles, effects, and its demands on other battle objects. Computed by
analysis, never authored. **Replaces the moveset context's *parity table*** — same role as the
single source of truth for a port, opposite provenance.
_Avoid_: parity table, spec sheet, move definition

**Measured value**:
A fact a port needs that provably is not in the game's data, recorded with a citation to the
recording it came from. The Alolan Whip's 20-frame rope hold is the type case: it belongs to the
rope article and appears in no param object. The only permitted hand-entered input, and a check
rejects one that later turns out to be derivable.
_Avoid_: override, constant, tuning value

**Feasibility class**:
How much of a port is mechanical, computed from the manifest rather than judged. **Class 1**
script-only, **class 2** script plus motion, **class 3** the move has its own status machine or
captures another fighter.
_Avoid_: difficulty, tier (that word is taken — see below)

## What the builder promises

**Support tier**:
The builder's declared stance on a fighter in a given role — *supported*, *supported with
caveats*, or *refused with a reason*. A fighter's tier differs by role: Kirby is a fine source and
a hazardous host, the Mii Fighters can source a move and cannot receive one.
_Avoid_: compatibility, whitelist

**CSK build**:
An output variant that may reference fighter-prefixed effect archives, and therefore depends on
the CSK Collection being installed. The default build uses only the always-resident `sys_*`
archive and depends on nothing.
_Avoid_: full build, faithful build
