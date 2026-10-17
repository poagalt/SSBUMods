# Open questions

A known gap is cheaper than a rediscovered one. Each entry says what is unknown, what it blocks,
what has been ruled out, and the cheapest route in. `BACKLOG.md` remains authoritative for work
items; this page is for the *facts* that are missing.

Closed items are kept at the bottom, because "we already tried that" is worth as much as the open
list.

---

## 1. The status-line dispatcher's return-value polarity

**Unknown:** what `main` does with the value a `CheckDamage` line returns.

**Blocks:** DK's carry damage rule — how an attack makes him drop the victim. The whole rule is
decompiled and every constant in it is resolved:

```
if !LinkModule::is_linked(boma, LINK_NO_CAPTURE)                        -> return false
elif damage["owner_object_id_"] == get_node_object_id(LINK_NO_CAPTURE)  -> return false
else
    reaction = DamageTransactorImpl::calc_reaction_frame(damage["reaction_max_"])
    level = reaction < get_param_float(boma,"battle_object","damage_level1") ? DAMAGE_LEVEL_1
          : reaction < ...("damage_level2")                                 ? DAMAGE_LEVEL_2
          : reaction < ...("damage_level3")                                 ? DAMAGE_LEVEL_3
          :                                                                   DAMAGE_LEVEL_FLY
    if level != DAMAGE_LEVEL_FLY
        if ItemModule::is_have_item(boma, 0) { ItemModule::drop_item(boma, 90.0, 0.0, 0) }
        return false
    else
        return true
```

`damage_level1/2/3` are on the **common** `battle_object` param object, so they read correctly off
any fighter. `DamageTransactorImpl::calc_reaction_frame(float)` is exported at `0x404ec0`.
`owner_object_id_` and `reaction_max_` came out of `main`'s own strings. smashline supports the
line (`StatusLine::CheckDamage`).

**Ruled out:** reading the polarity off the shape of the function. `true` is returned only on the
launcher path, which reads as "the damage applies" — but the same shape returns `false` when DK is
not carrying anyone at all, and "suppress damage whenever you are not carrying" cannot be right.
HDR's `MewtwoThrown_check_damage` returns 1 with the comment "bypass release set speed/status",
which points the other way.
`FighterStatusUniqProcessSwingGaogaenLariat_check_damage` looked like an oracle but is a
`FighterStatusUniqProcess` helper called from inside a status, not a registered line.

**Also open, same function:** Ghidra collapses `operator==`, `!=`, `<=`, `*`, and `=` into
`operator_`/`operator__`, so the comparison against `DAMAGE_LEVEL_FLY` is ambiguous in the C.
Only the mangled symbol distinguishes: `eqERKS0_` is `==`, `neERKS0_` is `!=`, `leERKS0_` is `<=`,
`ltERKS0_` is `<`, `mlERKS0_` is `*`, `ixEi` is `[](int)`, `cvbEv` is `operator bool`.

**Route in:** find the native caller of the line through `FighterStatusModuleImpl`'s vtable
(`disasm --dataxref` plus the vtable) and read what it does with the result. Or register the line
on **one** carry status in a build that only logs, and read whether the damage reaction ran.

Guessing costs either immunity to damage while carrying or an instant drop, both worse than the
current absence. ADR 0005 applies.

## 2. What the engine requires of a status to keep a capture alive

**Unknown:** the mechanism. Four of DK's `init_settings` and `set_fighter_status_data` arguments
were not being copied, and `FS_SUCCEEDS_KEEP_ATTACK_ABSOLUTE` is the leading candidate — but that
is inferred from the argument's *name*, not read.

**Route in:** if there is a per-status attribute table in `main`, reading it would answer this and
several other questions at once. With 100,508 entry points now decompilable
([data-index.md](data-index.md#function-boundaries)), this is cheaper than it was.

## 3. Why `CaptureModule::capture` returned without capturing — *narrowed 2026-09-14*

`is_capture` was false on the frame after the call, with `catch_object_id` still
`BATTLE_OBJECT_ID_INVALID`.

**No longer a mystery about the function, only about which gate.** `vtable[0x80]`'s target at
`main` `0x3ede00` is now decompiled (see
[grabs-and-victims.md](grabs-and-victims.md#making-and-unmaking-a-capture)) and has exactly two
failure paths: the `vtable[0x48]` pre-test returning false, or the `vtable[0xa8]` establish call
leaving its status byte zero, which makes the function call `vtable[0x68]` and return 0. Both
return 0 from `capture`, and **the port discards that return value.**

**Cheapest next step:** log `capture`'s return. A 0 says which class of failure happened and rules
out the other half of the space for free. The standing suspicion — that `capture` updates
bookkeeping for a link the CATCH system creates rather than creating one, so the missing call is
`CatchModule::set_catch` on the captor's side — is consistent with the `vtable[0x48]` pre-test
being the gate that fails, but is not yet evidence.

## 3b. The article clone crash — *root-caused 2026-09-14*

Five rounds blamed assets, then `vl.prc` params, then effects. All three were wrong, and the real
mechanism is one NULL pointer whose fault address is literally the struct offset that reads it.

```
0x3476e30  adrp x8, #0x52bc000
0x3476e34  ldr  x8, [x8, #0x3b0]      ; lib::Singleton<app::FighterParamAccessor2>::instance_
0x3476e38  ldr  x20, [x8, #0x1228]    ; entry[FIGHTER_KIND_PACKUN].param_object
0x3476e44  ldp  x8, x9, [x20, #0x138] ; *** Data Abort, Address 0x138, X[20] = 0 ***
```

`FighterParamAccessor2` holds an array of 94 entries of `0x38` bytes at `+0x60`, with each
fighter's param object at `+0x70 + 0x38 × kind`. The formula comes from a *named* symbol
(`app::ai_param::wire_range`) and was cross-checked against 23 offsets whose readers are named.
`0x70 + 0x38 × 0x51 = 0x1228` exactly, and `FIGHTER_KIND_PACKUN = 0x51`.

**Every entry is created in exactly one place, keyed on the fighter kind currently loading.**
`fighter_initialize_object_data` (`main 0x6079d0` — the same offset smashline hooks, with the same
five-argument signature) calls the creator at `0x70c580` under a per-kind refcount, and
`FUN_00709d60` clears all 94 entries at fight init. So with no Piranha Plant in the match,
`entry[0x51]` is NULL forever.

`clone_weapon` appends Piranha Plant's article descriptor verbatim — `weapon_id`, `on_init`,
`on_fini` — so `InstantiateBattleObject(1, WEAPON_KIND_PACKUN_SPIKEBALL, pikachu_entry)` creates a
genuine packun object, and **its native `WeaponSpecializer_PackunSpikeball` code runs**, compiled
against `+0x1228`. `use_original_code` is not only about which scripts load.

Consequences, all read rather than inferred:

- **No amount of asset, param, or effect work can fix this.** The project's three disproven
  hypotheses were disproven for the right reason.
- **There is no spare fighter-kind slot.** The array is 94 and the last kind is `TRAIL = 0x5D`
  (`FIGHTER_KIND_TERM = 0x5E`), confirmed three ways: the constant table, the clearing loop at
  `0x709d60` whose bounds are `0x60` to `0x14F0` — exactly `0x60 + 0x38 × 94` — and a 94-arm dense
  jump table whose entries were all read.
  **Correction, 2026-09-14:** an earlier version of this page said the creator "raises an
  undefined-instruction exception" for an out-of-range kind. That was a disassembly artifact. What
  sits at `0x70c580` in the dump is a **Skyline hook trampoline** — `ldr x17, #0x70c588; br x17`,
  because smashline hooks that exact offset (`sl_params.rs:305`) — and the `udf` after it is the
  high word of a 64-bit pointer, not an instruction. The original prologue survives at `0x70c590`,
  where the real `cmp w1, #0x5d` lives. **The out-of-range branch at `0x7153b0` is worse than a
  trap:** it skips construction, reads `entry[kind].param_object` at `+0x70 + 0x38 × kind` past the
  array's end, and tail-calls the initializer with it. Silent corruption, not a diagnosable crash —
  which matters, because "raises an exception" invites someone to expect a clean failure.
- **Article ids are not validated against a per-fighter maximum.** `generate_article` indexes
  `descriptor[id]` *before* comparing against the count, and an out-of-range id silently routes to
  the **item** path instead of erroring — which is the real reason the `0x6`-versus-`0x7` article
  id question was so hard to pin down.
- **smashline already hooks the creator** (`sl_params.rs`, offset `0x70c580`), but only to record
  which fighter is loading. It does not create entries for the original owners of cloned articles.
  That is a statement about the code, not a proposed fix.

**The one untested inference** is that the test match contained no Piranha Plant. The register dump
proves the slot was NULL and the code proves only a packun load fills it, but no roster log was
read. **Cheapest confirmation, and it needs no code:** log
`[[main+0x52bc3b0] + 0x1228]` from the plugin on Pikachu's first frame, once with a Piranha Plant
in the match and once without. Non-zero exactly when packun is present confirms the mechanism
observationally, with no crash and no clone enabled.

## 4. Palutena's n-air asymmetry

She animates the `Stick` bone with 460 keyframes of changing rotation and translation, and the
hitboxes live 19 frames, so **no static anchor can follow it.** Worst case 18.53 world units, and
the ceiling is recorded in `check-hitboxes.sh` so it cannot regress silently.

The user has deferred this with a direction: the real fix is implementing the actual weapon
objects rather than grafting bones. Chrom's jab, the same class of bug, is fixed exactly (0.00
units) because the sword's bone has a single transform.

## 5. Smaller open items

- **`SPECIAL_LW_TURN`** — the port ships `game_speciallwturn` but registers no status, so
  Incineroar's turn-during-Revenge is unported.
- **Ptooie / neutral B** — the only un-ported slot. **The `clone_weapon` crash is root-caused as
  of 2026-09-14**, off the 100,508-function decompile; see below. The remaining question is whether
  to act on it, not what it is.
- **`throw_invincible_frame` reads 0 on Pikachu** — the param is in none of the extracted files.
- **The whip has no ledge-grab box.**
- **No rope sprite.** Blocked by ADR 0002/0004; `extracted/fighter/gaogaen/motion/rope/c00/`
  exists locally, which is an unexplored angle. As of 2026-10-12, `generate_rope` has been read in
  full (the-article-path.md, "What `generate_rope` does after that read"). It ends in
  `generate_article(0, …)` behind a position gate, and article 0 on Pikachu is Thunder Jolt. So
  the rope needs a Pikachu-owned rope article plus the port doing the placement and the spawn.
  The spawn callback is `FUN_00abede0`: it builds the rope at the `top` joint from status floats
  7–8, stores its id in instance int `0x100000bf`, and raises `SPECIAL_S_GENERATED_ROPE`. That is
  the checklist of what a ported spawn must leave behind.
- **Call parity sits at 22 open entries**, every one reasoned. That is a reading list, not a defect
  list — see [parity.md](parity.md#why-call-parity-is-not-zero-and-why-that-is-correct).
- **Every ported move still plays PIKACHU'S sound, and the fix needs the console before it needs
  code.** The generator drops `sound_` along with `expression_` (gen-moves.sh: "Take only the game_
  function, rename it, drop sound_/expression_"), so Pikachu's own `sound_attack11` still runs under
  Little Mac's jab. Porting the source's script is one line in the generator and would be a
  REGRESSION on its own: `PLAY_SE(hash40("se_littlemac_attack100"))` resolves against the banks
  loaded for the fighter, and `se_littlemac.nus3audio` is not one of them — so the move would go from
  the wrong sound to no sound, which is the same ADR 0002 failure the effects had, in a form nothing
  in the harness can see.

  The order is therefore bank first, script second:

  1. **Pull the banks.** `tools/arc-pull/paths-sound.txt` lists 42 paths — `se_pikachu` plus the 20
     donors, `.nus3audio` and `.tonelabel` each — and every one was checked against the archive map.
     Pyra and Mythra are the exception worth knowing: their banks are PER COSTUME
     (`se_eflame_c00` … `_c07`), so the list pulls `_c00`, which is the rat's slot. Needs the game
     running, like every `arc-fetch` pull.
  2. **Merge additively into `se_pikachu.nus3audio`.** Entries are addressed by the hash40 of their
     label, so a donor entry copied in UNDER ITS OWN NAME is found by the donor's own `PLAY_SE`
     call. Nothing existing is touched, which matters because Pikachu's bank has no `cNN` variants —
     it is per-FIGHTER, so the gate in `src/charaslot` cannot reach it, and additive is the only
     shape that does not leak to vanilla Pikachu. A renamed-over entry would change HIS jab sound on
     all eight colours; a new entry he never asks for changes nothing.
  3. **Then emit the `sound_` scripts**, which can be checked the same way gate 3i checks effects.

  No nus3audio tooling is on hand (`vendor/tools/` has Ghidra and a JDK and nothing else), so step 2
  starts by acquiring it. That is the first thing to do that does NOT need the console.

---

## Closed, and worth not reopening

| Question | Answer |
|---|---|
| Does Incineroar's `SPECIAL_LW` register a `CHECK_DAMAGE` line? | **No. Nothing does.** His whole registration is 53 `sv_set_status_func` calls using four condition constants; `[x20, #0x3074]` is CHECK_ATTACK and is on his up-B alone. Revenge has pre, main, and end. So the counter's damage detection is native C++, and a frame callback reading `DamageModule::damage` deltas is not a workaround for a missed hook — it is the only hook there is |
| Can the const table's hash be inverted to name a constant? | **No.** `construct_bind_const_value_table` walks 19,469 precomputed 64-bit hashes in `lua2cpp_common`'s `.rodata`; neither XXH3-64 nor XXH64 with seed 0 reproduces a single one, and the registrar hashes `"<namespace>:<name>"` with a `0x3a` separator rather than the bare name |
| Can registration **order** index the table? | **No.** `main` registers 40,338 constants while the table holds a 19,469-entry subset, so index N is not registration N. (Both dead ends are now moot: the emulation reads name and value directly) |
| Is `main`'s `.data`/`.bss` worth dumping? | It was, and it is done. Dumping it *during play* crashed the game three times — the writes are synchronous through a 300 KB/s fs layer, and `.bss` is sparse |
| Is Blender required for appendages? | **No.** `ssbh_data_json` / `ssbh_data_py` round-trip `.numshb` to JSON with plain float arrays, and a raw cross-fighter `.nuanmb` drop-in needs no retargeting at all |
| Is there precedent for show/hide on a grafted mesh? | Yes — workshop episode 39. George cannot do it only because it reused an existing mesh-object name |
| Does `cancel_frame` get honoured? | Yes. A planned "retime" was abandoned after proving it |
| Does `ModelModule` have a parenting call? | **No.** `LinkModule::set_model_constraint_pos_ort` is the mechanism |
| Does Chrom have a sword article? | **No.** His sword is a mesh on his body model. `fighter/chrom/model/sword/` exists as geometry to merge, but there is nothing to clone |
| Does Chrom have a jab 2 or 3? | **No.** `c00attack12/13/100` do not exist. A "flow regression" was once recorded from inferring otherwise |
| Is the post-whip shrug ours? | **No, it is native** — `SPECIAL_S_FAILURE`, in 3 of 8 recorded native whips, identical animation (`0x11713d6de7`) and identical length |
| Does the victim run during the whip's rebound? | Yes, natively — `sub_get_rope_run_motion` |
| Is the point-blank whip nudge a bug? | **No** — two 0-damage `ATTACK`s on frames 15–16, with CATCH on 18–35 |
