# SSBU Mods

Super Smash Bros. Ultimate gameplay mods, built as ARCropolis file replacements plus a
Skyline plugin. No dependency on shared moveset frameworks.

## Language

### The thing being built

**Moveset mod**:
A mod that changes what a fighter's moves *do*, not just how they look.
_Avoid_: skin, costume, character mod

**Host fighter**:
The vanilla fighter whose slot, model and skeleton the mod runs on. Here: Pikachu.
_Avoid_: base character, target

**Source fighter**:
The vanilla fighter a single move is taken from. Each slot names its own.
_Avoid_: donor, original

**Slot**:
One entry in a fighter's movelist (jab, f-tilt, up-B, pummel...). Not to be confused with
*costume slot*.
_Avoid_: move, input

**Costume slot**:
One of a fighter's eight alternate outfits, `c00`–`c07`. Always written with the `cNN` form.
_Avoid_: slot, alt, skin

### Fidelity

**Parity**:
A ported move behaving identically to the source fighter's version. Scope of "identically"
is decided per-axis (frames, damage, knockback, reach) — see open questions.
_Avoid_: accuracy, faithful, 1:1

**Reach**:
How far a hitbox extends in world space from the fighter's origin. The quantity that must be
preserved when porting a move onto a smaller skeleton.
_Avoid_: range, length

**Appendage**:
A limb or held object shown on the host fighter only while a ported move is active, so the
move visually accounts for its reach. Removed when the move ends.
_Avoid_: prop, attachment, bolt-on

**Disjoint**:
A hitbox with no hurtbox behind it. Swords and other held weapons are disjoint; ported limbs
are not.
_Avoid_: disjointed hitbox, sword hitbox

### Implementation vocabulary

**Agent**:
Smashline's term for the script owner of a fighter or one of its articles. Reserve this word
for that meaning; a Claude subagent is a **subagent**.
_Avoid_: using bare "agent" for an AI subagent

**Article**:
A fighter-owned entity with its own model and scripts (Piranha Plant's Ptooie ball, G&W's
parachute, projectiles). Distinct from an appendage, which is part of the host's model.
_Avoid_: projectile, entity, spawn

**Motion**:
An animation entry, declared in `motion_list.yml` and backed by a `.nuanmb` file.
_Avoid_: anim, animation file

### Fidelity, continued

**Move property**:
A non-hitbox behaviour belonging to one specific move — an armor window, a limb intangibility
window, an autocancel point. Ports with the move.
_Avoid_: special property, quirk

**Character-wide trait**:
A property the source fighter carries across their whole moveset (K. Rool's belly armor, Terry's
special-cancel, Cloud's Limit gauge). Does **not** port. See ADR 0001.
_Avoid_: passive, universal trait

**Parity table**:
The single checked-in machine-readable record of every hitbox's source-of-truth values. Move
scripts are generated from it, so it is the only place a value is edited by hand.
_Avoid_: hitbox data, spec sheet
