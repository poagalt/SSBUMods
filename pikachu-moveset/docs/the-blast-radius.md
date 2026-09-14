# Does this mod touch anything but Pikachu?

Audited 2026-09-14, after native Donkey Kong's cargo throw crashed the game. Every surface the mod
has for reaching outside Pikachu, with what was found at each.

## The crash, stated correctly

`retarget_forward_throw` wrote `RAT_STATUS_KIND_SHOULDER_START` (0x1F7) into `globalTable[0x45]`,
the forward-throw destination, every frame -- from `telemetry::pikachu_telemetry`, a Main line
callback installed on `SOURCE_AGENTS` (gaogaen, donkey, gamewatch, packun) as well as on Pikachu.

My first account of the consequence was wrong. I said DK "entered a status he has no scripts for".
He has scripts for 0x1F7: it is **`FIGHTER_DONKEY_STATUS_KIND_THROW_F_HI`**, his cargo throw UP.
His SHOULDER_START is 0x1EA. So his forward cargo throw ran his throw-UP script, reached from a
catch status rather than from the carry, and `LinkModule::send_event_nodes_throw` fired against a
grab that status had not set up -- which is the last line in pt36 before the crash.

## What the audit found beyond it

**The guard was the wrong shape.** Fixing that one call left fifteen others running per frame
against four foreign fighters. The gate is now one `mine` test at the top of the callback and every
MUTATION is inside it; reads and log lines still run for everyone, because tracing the real DK is
what the source agents are for.

**And there is a second hazard that is not hypothetical.** Every static in this plugin is indexed
by ENTRY -- the player SLOT -- and a slot outlives a fighter. pt37 contains `SELFTEST p0` for
Pikachu and `DONKEY0` for Donkey Kong, in one log, because the character in slot 0 changed between
matches. Nothing resets `crate::victim`, so a `CUT_ID[0]` armed by a Pikachu is still armed when a
DK spawns into slot 0, and `tick_deferred_cut` would resolve that stale battle-object id and act on
whatever now answers to it. Same exposure for `speciallw`'s PHASE and RATE and `shoulder`'s VICTIM.
The `mine` gate closes it; a per-object reset would close it better and is not done.

**The telemetry was naming foreign fighters' statuses out of Pikachu's table, and it cost a wrong
diagnosis.** pt36 reads `DONKEY0 status ... -> RAT_SHOULDER_START(0x1f7)`, which is how I first
concluded DK had been pushed into one of Pikachu's statuses. `statusnames::of_own` covers
0x1F0..0x206 and those ids belong to whoever is running: DK's 0x1F0 is SHOULDER_JUMP, 0x1F1 is
SHOULDER_FALL, 0x1F7 is THROW_F_HI. `probe::st_kind(k, ours)` now gives a foreign fighter the
COMMON table only and prints a bare number past it.

**The engine tracer had been replacing seventeen engine functions in every match since f57b006.**
Fixed separately in 3ed6c6c; it is an instrument and its default is off.

## What was checked and is clean

| Surface | Finding |
|---|---|
| `skyline::hook` replacements | 4 in `params`, all gated `id/object == <const> && is_pikachu(boma)` with `&&` short-circuit, so a foreign call never reaches `is_pikachu`. 17 in `enginetrace`, now not installed. |
| `Agent::new(<name>)` | Every one is `"pikachu"` except `telemetry`'s `SOURCE_AGENTS`, which installs a Main line callback and nothing else. |
| ACMD installs | All on the pikachu agent; animcmd scripts are per-fighter. |
| `fighter/common/param/fighter_param.prcxml` | Declares indices 0..93 but only index 7 (Pikachu) is a `<struct>`; the other 93 are `hash40 "dummy"` placeholders. Vanilla is 94 structs of 369 params, so those would be catastrophic if applied -- and they are not. Proof is runtime, not reasoning: vanilla Pikachu `walk_speed_max` is 1.302, ours is 1.575, and pt37 reads **1.5750** while every other fighter plays normally. The patch applies AND the placeholders are inert. |
| `fighter_param_motion.prcxml` | One `<struct>`, index 7. |
| `config.json` | Every path under `fighter/pikachu/`. |
| `ui/message/msg_name.xmsbt` | Five entries, all `*_pikachu`. |
| `clone_weapon` | `CLONE_ENABLED = false`, so not called; and it clones packun's weapon INTO pikachu, which reads packun without changing it. |
| Writes through a victim accessor | Six, all in `whip`/`shoulder`/`victim`, all inside Pikachu's own statuses -- and the whip and carry must move their victim. None reachable from the shared callback any more. |
| Raw memory patching | None outside `exedump` and `memserve`, both `ENABLED = false`. |
| `lockout::install()` | Pre/Main/End on retired kinds, on the pikachu agent. |

## One cost that is not a bug

The four `params` hooks REPLACE `WorkModule::get_param_int`, `get_param_float`, `is_flag` and
`get_float` for the whole game. They are correct -- each begins with an id or object-hash compare
that is false for every call in the game except Skull Bash's and Incineroar's own -- but every
param read by every fighter now goes through one extra comparison. That is the documented price of
answering the victim's status without shipping a `vl.prc`, and `params/mod.rs` records why the
file was rejected (a null dereference in smashline's effect loader on fighter load).
