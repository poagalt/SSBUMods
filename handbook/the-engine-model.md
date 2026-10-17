# What a move is to the engine

A move is not one thing in one file. It is five layers, each in a different format, in a different
place, and a port that gets four of them right still fails. This page is the map: which layer
holds which fact, and what it costs to get it wrong.

## The five layers

| Layer | Lives in | Holds |
|---|---|---|
| **Status** | compiled L2C in `lua2cpp_<fighter>.nro`; victim-side in `lua2cpp_common.nro` | the state machine: what state the fighter is in, what it may transition to, kinetic mode, per-frame logic |
| **Motion** | `fighter/<f>/motion/body/cNN/motion_list.bin` | animation binding, **FAF**, **intangibility**, root-motion flag, blend |
| **ACMD** | script functions in the agent, replaced via smashline | hitboxes, grab boxes, timing, effects, sound, per-move speed |
| **Params** | `fighter/common/param/*.prc`, `fighter/<f>/param/vl.prc` | tuning numbers the status and ACMD code reads by hash |
| **Engine** | compiled C++ in `main` | capture, damage, knockback, hitlag, the catch handler, everything keyed on fighter kind |

The single most expensive misunderstanding in this project's history was assuming fighter
behaviour lived in the first four. It does not. "When this catch connects, put the victim there
and me here" is layer five, keyed on the fighter kind, which is why six moves started life as
reimplementations rather than drag-ins.

## Layer 1 — status

A status has **lines**, not one function. smashline supports seventeen: `Pre`, `Main`, `End`,
`Init`, `Exec`, `ExecStop`, `Post`, `Exit`, `MapCorrection`, `FixCamera`, `FixPosSlow`,
`CheckDamage`, `CheckAttack`, `OnChangeLr`, `LeaveStop`, `NotifyEventGimmick`, and `CalcParam`.

- **`Main` runs ONCE at status entry**, not per frame. Per-frame logic is the `main_loop` reached
  through `sub_shift_status_main`. A hook written on `Main` expecting per-frame work is inert.
- **Which lines a status has is a fact you can look up, not infer.** In
  `vendor/ssbu-status/ghidra_scripts/lua2cpp_<f>.nro/<f>/<f>/status/<Status>/`, there is one `.c`
  per line the status actually has — **the presence of `exec.c` IS the fact that it has an Exec
  line.** Read that directory before disassembling anything.
- Missing a line is silent and total. DK's `SHOULDER_WALK` has three lines; the port registered
  Init/Pre/Main/Exit and no `Exec`, so `sub_walk_uniq_process_main_common` never ran after frame
  0, the motion frame stayed pinned, and the carry walk moved `+0.00` units while looking alive.

### The global table

Status code reads the fighter's global table by index. The ones that matter:

| Index | Meaning |
|---|---|
| `[5]` | module accessor |
| `[9]` | status kind |
| `[0xa]` | previous status |
| `[0xb]` | **next / incoming status** |
| `[0xc]` | status-kind count (smashline writes this to extend the table) |
| `[0xe]` | status frame |
| `[0x15]` | per-frame sub-status callback |
| `[0x16]` | situation kind |
| `[0x17]` | previous situation |
| `[0x20]` | pad cat1 flags |
| `[0x21]` | pad cat2 flags |
| `[0x45]`–`[0x48]` | **the four throw destination statuses** (f/b/hi/lw), read by `L2CFighterCommon::CatchCont` |

`[0x45]`–`[0x48]` are how a cargo carry is entered natively: DK writes `SHOULDER_START` into
`[0x45]`, so the engine's own catch-continue logic sends the victim there instead of to a throw.
That is a one-line retarget and it replaces an entire hand-written interception.

### Status kinds, and adding your own

Status kinds are per-fighter enumerations above the common block. Pikachu's own extras run
`0x1E1` `SPECIAL_HI_WARP`, `0x1E2` `SPECIAL_HI_END`, `0x1E3` `SPECIAL_S_HOLD`, `0x1E4`
`SPECIAL_S_WEAK`, `0x1E5` `SPECIAL_S_ATTACK`, `0x1E6` `SPECIAL_S_END`, `0x1E7` `SPECIAL_LW_HIT`,
`0x1E8`–`0x1EF` Final Smash, with `NUM = 0x1F0`.

**smashline extends the table**: `install_status_scripts` tracks `max_new = max(status.id + 1)`
and writes `global_table[0xC]`, so new kinds go past the host's `NUM`. This project registers 22
of them at `0x1F0`–`0x205`, with `RAT_STATUS_KIND_NUM = 0x206` and a compile-time assert that
every id sits below it. Verified on hardware.

Reusing a host status kind for a ported move does not work: the host's own Init and Exec still
run. Remapping G&W's up-B onto Pikachu's `SPECIAL_HI_WARP` produced Quick Attack's teleport and
invisibility on top of the port.

### Capture statuses, by number

They appear as raw numbers in traces, so know them: `0x41` `CAPTURE_PULLED`, `0x42`
`CAPTURE_WAIT`, `0x44` `CAPTURE_CUT`, `0x45` `CAPTURE_JUMP`, `0x37` `CATCH`. Command-grab victim
statuses are **engine-common**, not fighter-scoped: `SWING_GAOGAEN_*` is `0x176`–`0x17C` and
`SHOULDERED_DONKEY_START/_/THROWN` is `0xC5`/`0xC6`/`0xC7`.

A common status is not fighter-agnostic just because every fighter can enter it. ADR 0004 exists
because `status_SwingGaogaenThrown` fetches its **linked parent's** position and reads x, y, and z
off the returned pointer with no null check — for Incineroar the parent is always his rope
article, so on a fighter with no rope the pointer is null and the third load is the fault address.

## Layer 2 — motion

`motion_list.bin` is a per-costume binary; `motion_list.yml` is the round-trip YAML of it. Each
entry:

```yaml
attack_11:
  game_script: game_attack11
  flags: { turn, loop, move, fix_trans, fix_rot, fix_scale, unk_* }
  blend_frames: 0
  animations: [{ name: c00attack11.nuanmb, unk: 0 }]
  scripts: [expression_attack11, sound_attack11, effect_attack11]
  extra: { xlu_start: 0, xlu_end: 0, cancel_frame: 21, no_stop_intp: false }
```

- **`cancel_frame` is FAF.** It can only permit an *earlier* exit. An attack status ends when the
  **animation** ends (`if MotionModule::is_end { change_status(WAIT) }`), so no `motion_list`
  value can make a move longer. To lengthen, use `FT_MOTION_RATE_RANGE`.
- **`xlu_start` / `xlu_end`** is the intangibility window.
- **`move`** is animation root motion. When true the engine converts the `Trans` track to
  velocity; when false it sets max speed 0 and brakes with `ground_brake`. Inheriting a source
  fighter's whole flag word drags `move`, `turn`, `loop`, and the `fix_*` set along with it — 48
  units of unwanted dash-attack travel came from exactly that. Patch `cancel_frame` (and `xlu_*`
  where a status reads it) and leave the rest.
- **`blend_frames`** is how many frames the engine cross-fades *into* a motion. It is the
  "seamless transition" requirement expressed as a number, and it is forced to 0 on borrowed
  animations so the host's rest pose does not bleed in.
- **Animation filenames are universal per motion across every fighter** (verified 11/11, then
  34/36): `c0*` normals, `d0*` specials, `e0*` grab and throw, `j01` taunts. The prefix is the
  game's own category convention and encodes nothing about the source fighter. This is what makes
  a raw cross-fighter `.nuanmb` drop-in work at all — the file lands in the host's own directory
  under the host's own filename, so no cross-fighter file sharing and therefore no CSK dependency.
- There are **no transition-term fields** in `motion_list`. Transitions are
  `WorkModule::enable_transition_term_group` / `unable_transition_term_group_ex` plus the
  `sub_transition_group_check_air_*` helpers, in the status code.

## Layer 3 — ACMD

One `Agent` per fighter, four script categories per move: `game_`, `effect_`, `sound_`, and
`expression_`. Stubbing only `game_` leaves the host's VFX and SFX firing; `acmd_stub` exists in
smashline for this and should be applied to all four.

Hitboxes are `macros::ATTACK(agent, id, part, bone, damage, angle, kbg, fkb, bkb, size, x, y, z,
Some(x2), Some(y2), Some(z2), …)`.

- **Offsets are bone-local for a named bone, and fighter-local for `top`.** `top` is not a
  skeleton bone; it is the fighter's origin. For `top`: **x = depth (≈0), y = up, z = forward**,
  with facing applied by the engine.
- **Liveness invariant: `|x| <= size`.** A hitbox only connects if it intersects the x≈0 plane the
  fighters stand on. Vanilla honours this in 98% of its 1,660 non-zero-depth top-anchored
  hitboxes. Violating it produces geometry in the background that can never hit anything, which is
  what re-anchoring a bone-local offset into `top` space did on 14 of 53 capsule far ends.
- **Per-bone axis conventions differ between fighters**, which is why
  `AttackModule::set_attack_reference_joint_id(boma, bone, ATTACK_DIRECTION_Z, …)` exists. Do not
  transplant bone-local offsets across fighters.
- **`ATTACK_ABS` / `ATK_HIT_ABS` are not absolute-position hitboxes.** Every instance in HDR and
  in seven published movesets is `FIGHTER_ATTACK_ABSOLUTE_KIND_THROW` or `_CATCH` — throw and grab
  collisions with no position arguments. `top` plus an offset is the only positional mechanism.
- **`FT_MOTION_RATE(r)` is GAME FRAMES PER ANIMATION FRAME**, so `0.25` is four times faster. It
  falls out of `FT_MOTION_RATE_RANGE(start, end, game_frames) = game_frames / (end - start)`.
  Always bracket a rate change with a reset to `1.0`; the corpus has no counter-example, and two
  published movesets guard against it explicitly at the top of every script.
- `MotionAnimcmdModule::flush(boma, false)` on a motion change drops a stale ACMD coroutine. That
  is the antidote to a truncated or re-entered move leaving hitboxes live.

## Layer 4 — params

Three scopes, and the scope decides whether a port has to do anything:

| File | Scope | Note |
|---|---|---|
| `fighter/common/param/common.prc` | **game-wide, one value for every fighter** | `turn_stick_x`, `walk_stick_x`, `turn_frame`, `pass_stick_y`, `jump_stick_y`, `throw_invincible_frame` … portable with nothing to provide |
| `fighter/common/param/fighter_param.prc` | per fighter, one row each | `fighter_param_table`, 94 rows, resolved by each row's own `fighter_kind` field. Pikachu is index 7 |
| `fighter/<f>/param/vl.prc` | per fighter, per move and per article | `param_special_s`, `param_lifting_walk` … a ported move's tuning lives here, on the **source** fighter |

- **`WorkModule::get_param_float(boma, object, key)`** — the first hash is the param *object* and
  the second is the *key*. `get_param_float(boma, hash40("common"), hash40("turn_stick_x"))` reads
  a shared threshold; `get_param_float(boma, hash40("walk_speed_max"), 0)` reads a top-level
  per-fighter value. Swapping them returns 0 silently, and a `.max(0.1)` floor downstream then
  hides it — the carry's turn and walk triggered on the literal `0.1` for several rounds that way.
- A ported move that reads its source's `param_<move>` object gets nothing on the host, because
  the host's `vl.prc` has no such object. Two answers: hardcode the values as constants, or hook
  `get_param_int`/`get_param_float` and answer the whole object for the host
  (`src/params/`, generated by `tools/gen-paramprovider.sh`). `get_param_reimpl` **falls through**
  to the original lookup on a miss, so a missing article param does not crash.
- **Storage type and accessor are independent** — the engine coerces `get_param_int` and
  `get_param_float` either way.
- `.prcxml` must be **UTF-8** (ARCropolis uses `prcx` + `xml-rs`); `.xmsbt` must be **UTF-16LE
  with a BOM** (msbt is a UTF-16 container). Getting this backwards means the file parses as
  nothing and applies silently.
- Aerial landing lag is `landing_attack_air_frame_{n,f,b,hi,lw}`, a param — patching a
  `landing_air_*` motion entry's `cancel_frame` is not the same thing. Dodge timing is
  `fighter_param_motion.prc` (`escape_*_cancel_frame`, `escape_air_slide_speed`/`_distance`), not
  `motion_list`.

## Layer 5 — the engine

What is only here, and therefore only reachable by reading `main`:

- **Capture.** Two-sided:
  `CaptureModule::capture(victim_boma, captor_id, STATUS_KIND_NONE, true, 0)` on the victim, plus
  `LinkModule::link(boma, LINK_NO_CAPTURE, victim_id)` on the captor.
  `LinkModule::link` writes only the **caller's** slot and splices the caller's two lists; it does
  not touch the named object. `CaptureModule::is_capture` is **true on the captor**, not only on
  the capturee. `Capture` itself is virtual (`vtable[0x80]`), so it has no static `bl` caller.
- **Releasing it.** `CatchModule::catch_cut` is four instructions — `this->[0x93] = 1` — a
  *request*, consumed at the engine's next update only if a catch is active. Raised with nothing
  to cut, it survives to the next catch, which is the "cannot grab again until the game's own
  reset" bug; a training-mode save-state restore does not clear it.
  `CaptureModule::capture_cut(boma, p2, p3, p4)` gates `vtable[0x3b8]` — the call that actually
  undoes the capture — on **`p2 && p3 && !p4`**, and `vtable[0x3c0]` on `p2` alone. The correct
  call is `capture_cut(v, true, true, false)`. Field `+0x78` is the capture flag (`IsCapture` at
  `0x3edbd0` is its *setter*); field `+0x7d` gates both `CaptureCut`'s teardown and `Thrown`.
- **Escaping it.** Four lines of `CaptureWaitCommon`: `ControlModule::get_clatter_time`, then
  `is_enable_transition_term(FIGHTER_STATUS_TRANSITION_TERM_ID_CAPTURE_CUT)`, then
  `change_status(CAPTURE_CUT)`. The mash-out is a clatter timer gated by a transition term the
  victim's own `status_CaptureWait` enables.
- **The grip is free.** `LinkModule::set_model_constraint_pos_ort` binds the victim's `"rot"` to
  the captor's **`"throw"` bone** the moment a capture exists. Hand-positioning a held victim is
  not necessary, and doing it is what glued opponents to Pikachu's feet.
- **Hitlag:** `floor((d × 0.65 + 6) × h × e × s × p × c)`, capped at 30.
- **Battle object id:** `category = id >> 28`, `index = (id >> 16) & 0xFFF`. Entry 0 is id `0x0`,
  which is valid; `BATTLE_OBJECT_ID_INVALID` is `0x50000000`.
- **`unlink` on an already-empty link is a crash** (fault address 0, inside `cross2 + 0x47f4a0`),
  and forcing a victim into the status it is already in is a crash (re-entry re-runs the entry
  path).

## Numbers worth knowing

Facts about the game itself that took measurement to establish and are cheap to look up here.

- **Get-up attacks are almost entirely standardised**: 45 total frames for all 89 fighters with
  zero variance, 7% damage for 86 of them, and all 178 variants hit both sides. The only real
  levers are where intangibility ends and how far the hitbox reaches (Bowser's is 54.5 units
  against 51 for second place and a 38 median). Frame-data sites do not publish this at all — it
  comes out of the game's own scripts.
- **Fighter `scale`** is a real `fighter_param` key, one per fighter: Pikachu 0.96, Chrom 0.96,
  Palutena 0.95, Cloud 0.98, King K. Rool 0.89, Corrin 0.99, Kazuya 1.007, Falco 1.05, Fox 1.08,
  Little Mac 1.11, Mythra 1.112. Generate it from the game (`tools/gen-scales.sh`); two hand-copied
  constants were wrong by 5% and 29%, and the hitbox trace could not see it because it applied the
  same wrong constant to both sides of its own comparison.
- **Bind-pose head heights**, for reach intuition: Pikachu 7.20, Fox 11.84, Falco 13.87, Little Mac
  14.62, Mythra 14.98, Corrin 16.25, Palutena 17.48, Chrom 17.50, Kazuya 17.87. Pikachu's head
  sits at y = 7.37 in practice, which is why hitboxes re-anchored to a source's absolute height
  left a ten-unit dead zone above him and read in a playtest as "seems too small".
- **`dive_speed_y`** is the fast-fall parameter — the hash the reference mod's teardown could not
  name (Joker 3.097, Zero Suit Samus 2.72, Pikachu 2.48).
- **G&W's up-B `stick_mul` is 30 *degrees*** of launch-angle deviation, not a speed. His `exec`
  computes `sign(stick_x) × 30` and applies it with `set_rot` **and**
  `sv_kinetic_energy::set_angle(MOTION, math_rad(angle))`; his `SpecialHi_end` then resets rotation
  to zero **unconditionally, before any early return** — put that reset where it "naturally" goes,
  after the landing-lag checks, and the lean survives the move on most paths.
- **Hitlag** is `floor((d × 0.65 + 6) × h × e × s × p × c)`, capped at 30.
- **Capsule hitboxes** use the game's own stretch arguments, `Some(x2), Some(y2), Some(z2)`;
  R.O.B.'s d-tilt is a vanilla example.

## The two naming systems

Everything above is addressed by one of two encodings, and neither is readable on sight.

**hash40** — `(len << 32) | crc32(lowercased_bytes)`. Used for motions, params, bones, and
effects. `tools/hash40.sh` goes both ways and `--len N` narrows a guess, since the length is in
the top byte. Three dictionaries cover 149,871 community names, 14,629 ACMD literals, and 47,856
strings harvested from `main`'s own `.rodata` and `.text`. The third is the only one with C++
member names like `owner_object_id_` and `reaction_max_`, which is what DK's carry damage rule
reads out of the damage table.

**lua_const** — one i32 array, `lua2cpp::L2CAgentGeneratedBase::const_value_table__`, that all
status code reads as `ldr wN, [xTABLE, #off]`. A status kind, a work id, a flag, and a link
number all look identical in a disassembly and carry no name. **This is now fully resolved for
13.0.5**: all 19,310 offsets have a name and a value
([data-index.md](data-index.md)), so
`ldr w1, [x24, #0x250]` reads as `STATUS_KIND_NONE = 0xFFFFFFFF`.

A **status-scoped work id is an array index, not a name.**
`FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLOAT_ROPE_X = 0x01000007` means "float 7 of whatever status the
object is currently in". Answering one globally fed a world coordinate into every other status's
float 7 and produced "pikachu running really fast", erratically, surviving resets. Instance work
ids collide the same way: every fighter numbers its own from the end of the common block
(`FIGHTER_INSTANCE_WORK_ID_FLAG_TERM = 0x200000E0`), so
`FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLAG_INVALID_SPECIAL_AIR_S` **is**
`FIGHTER_PIKACHU_INSTANCE_WORK_ID_FLAG_PAUSE`.

`WorkModule`'s INT and INT64 work ids are **separate arrays** — `set_int64` on an INT id writes
the wrong variable, self-consistently, so a write and its read-back agree while the engine sees
nothing.
