# Auditing every move against the game's own logic path

Asked for while the data pull ran: *"do a deep audit of each move's implementation to native.
Carefully make a list of all possible edge cases beforehand... The ideal state for each move would
be to use the game's own logic path exactly for it."*

## What makes a move exact, and the three ways it can fail

A ported move is exact when the engine runs the same code for Pikachu that it runs for the source
fighter. There are exactly three places that can break, and the project has now been bitten by
all three:

1. **The ACMD and animation.** Covered by construction under ADR 0003 -- Pikachu plays the
   source's own animation, so bone-anchored hitboxes land where they landed. Gated by
   `check-hitboxes.sh` (38 moves, 460 hitbox-frames, 0.05 world units).
2. **The status lines.** Covered by ADR 0006 and `check-statuslines.sh`: every Init/Exec/Exit the
   source has is registered, and every new status registering `End` also registers `Exit`.
3. **The calls inside those lines.** *Not covered until now.* A status line can be registered,
   named correctly and still omit half of what the source's does.

Failure 3 is what the cargo landing slide was: `ThrowFF_main` shifts to a loop that is one call
to a static, `FUN_7100021af0`, and that static swaps the kinetic to `GROUND_STOP` the frame DK
lands. The community status dump contains only status LINES, so the static was invisible; the
port kept `UNIQ` and slid.

`tools/check-callparity.sh` closes that: for each ported move it takes the union of the source's
status directories **and every `FUN_71xxxxxxxx` static those directories call**, and reports
engine calls the source makes that appear nowhere in the port.

## Edge cases to check per move, before reading any code

Written down first, deliberately, because every round of this project that went wrong went wrong
on a case nobody had listed.

**Situation and transition**
- entered on the ground vs in the air
- becoming airborne mid-move (walked off a ledge)
- **landing mid-move** -- the cargo slide; kinetics must follow the situation every frame
- interrupted by damage, by a grab, by death, by the stage
- the move's own status ending vs the engine ending it
- what the NEXT status is: DK's carry exits and gaogaen's whip release both branch on
  `globalTable[0xb]`

**Facing**
- used facing left vs right (the Chrom jab / Palutena n-air mirror class)
- `reverse_lr` and `update_rot_y_lr`, and their real trigger -- gaogaen's failure turns around
  only when the next status is `DOWN_SPOT`, not on every grounded whiff

**Grabs specifically**
- who established the capture: the engine, or this port by hand
- the victim mashing out vs being thrown vs the captor being hit
- **grabbing again afterwards** -- five rounds of this
- the victim being player one (`id 0`, which three arrays in this repo used as "nobody")
- a second grab attempt while a stale record is live

**Resources the port does not ship**
- articles (ADR 0004): the whip's rope, G&W's parachute
- CSK effects (ADR 0002)
- items in hand: **DK drops his held item when he starts the carry** -- found by this audit

**Numbers**
- every param read answered for Pikachu rather than defaulting to 0
- frame counts in the unit they were measured in (animation vs game frames, and motion rate)

## The audit's findings

36 real gaps after filtering the decompiler's `_impl` suffix and scoping each fighter's statics to
the status group that calls them. Both filters mattered: unfiltered the report was 253 entries,
which is a report nobody reads twice.

| module | source | calls | missing | verdict |
|---|---|---|---|---|
| whip | gaogaen `SpecialS*` | 43 | 5 | investigate |
| shoulder | donkey `Shoulder*` | 33 | 3 | **one real behaviour** |
| shoulder | donkey `ThrowF*` | 15 | 2 | investigate |
| speciallw | gaogaen `SpecialLw*` | 17 | 5 | investigate |
| specialhi | gamewatch `SpecialHi*` | 28 | 15 | 6 are ADR 0004 articles |
| specialhiclose | gamewatch | 8 | 2 | both ADR 0004 articles |
| specialhifall | gamewatch | 14 | 4 | all ADR 0004 articles |

**Correct absences.** Every `ArticleModule::*` is G&W's parachute, which ADR 0004 excludes -- 12
of the 36. They are the decision working, not a defect.

**`ItemModule::is_have_item` / `drop_item` -- and my first reading of it was wrong.** I wrote that
DK drops his item when he STARTS the carry. He does not: both calls are in `check_dmg.c`, the
CheckDamage line, on all ten carry statuses. He drops it when he TAKES A HIT at DAMAGE_LEVEL_FLY
while carrying.

That makes it part of a bigger absence than a one-line transcription: **this port registers no
CheckDamage line at all.** DK's is substantial -- it asks whether the damage came from the fighter
he is carrying (`get_node_object_id` against the attacker id), runs the reaction frame through
`DamageTransactorImpl::calc_reaction_frame`, compares it to a param, and only then decides. Being
hit during a carry is governed by code this port does not have, and the item drop is one visible
consequence of that.

**Worth reading next**, in rough order of how likely they are to be visible:

- whip: `MotionModule::rate` and `set_frame`. The lariat interpolates its animation speed by how
  far into the window the button came, and the port uses `set_rate` but never reads `rate` or
  sets a frame. If the interpolation has a second half, this is where it is.
- speciallw: `WorkModule::on_flag` / `off_flag` / `is_flag`. Revenge's stance is flag-driven and
  three flag calls with no counterpart is the shape of a missing state transition.
- shoulder: `StatusModule::is_changing`, in both groups. A guard against acting during a status
  change, and its absence is the kind of thing that shows up as a one-frame glitch.
- specialhi: `KineticModule::change_kinetic` and `WorkModule::enable_transition_term_group`.

None of these are being fixed blind. Each one gets the same treatment the slide got: read the
source function, check what the port does at that point, and only then change something.

## Second pass: reading all of them

Every entry now has a reason. The headline is that **most of them were my tool's fault, not the
port's** -- two separate false-positive sources, each worth more than the gaps they hid.

**The `_impl` suffix.** `lua_bind::MotionModule__frame_impl` is the decompiler's name for the
thunk; the port calls the wrapper, `MotionModule::frame`. Unfiltered, all 253 calls looked
missing.

**Statics belong to the status that reaches them.** Concatenating a whole fighter's statics into
every status group made Revenge look like it was missing the whip's release.

**A glob that over-matched.** `SpecialHi*` matches SpecialHi, SpecialHiClose and SpecialHiFall,
but the port splits G&W's up-B across three modules -- so every call made by Close or Fall was
reported missing from `specials/specialhi`, which does not implement them. Nine false positives
from one asterisk. The map takes a trailing `=` for an exact directory now.

253 -> 36 -> 23.

### Fixed

| what | where it was |
|---|---|
| cargo throw kept air momentum on landing | `FUN_7100021af0`, a static: swap to `GROUND_STOP` |
| ...and the swap is a TRANSITION behind `is_changing` | same function, read further |
| lariat stayed slow for its whole length | `SpecialSLariat_main_loop` restores rate at the hit frame |
| Revenge kept the air animation after landing | `SpecialLw/main_loop` pairs `correct` with `change_motion_inherit_frame`, both ways |
| Revenge could not be cancelled at its FAF | first statement of `SpecialLw/main_loop` |

### Correct absences, each checked rather than assumed

| what | why |
|---|---|
| 19 x `ArticleModule::*` | ADR 0004 -- G&W's parachute and rescue, gaogaen's rope |
| `StopModule::is_stop` (specialhi) | gates `FUN_710002cab0`, which is *entirely* parachute |
| `WorkModule::on_flag` (specialhi) | `FLAG_SPECIAL_HI` is a G&W INSTANCE work id; on Pikachu it aliases his own block, so setting it is a hazard, not fidelity. Read only by G&W's own end line and a stage gimmick |
| Revenge's 3 STANCE flag calls | `FLAG_STANCE` is touched only in `SpecialLw/main_loop.c` and read nowhere else in the module or its statics. The ACMD calls `open_window`/`close_window` at the same frames |
| whip `KineticModule::get_sum_speed_y` | **dead in the source** -- read into `auStack_90` at line 51 of `FUN_710001d4b0`, destroyed at 742, never used |
| whip `WorkModule::get_param_int` | the clatter trio. Gaogaen's helper reads them; here the ENGINE reads them through `src/params` and the traced result is identical, `start_clatter(85, 0, 5)` |
| whip `WorkModule::set_float` | `LARIAT_AREA_HIT_POS_X` / `_BUTTON_PUSH_POS_X`, gaogaen INSTANCE ids that alias Pikachu's float block. `state.rs` stores both |

### Still open, and now specific

~~**`param_special_hi`: `stick_min`, `stick_mul`, `landing_frame`.**~~ **NOT A GAP -- I was wrong
again.** The provider answers only `param_special_s` at runtime, which is what made these look
unanswered, but every other param object is HARDCODED in Rust and `check-srcparams.sh` verifies
the constants against the same prc. For gamewatch it reports **5 used, 0 declared unmodelled, 0
unaccounted**: stick_min 0.25, stick_mul 30, limit_y_mul 0.5, landing_frame 40,
pass_speed_mul 1.1. See `the-data-analysis.md` for why this keeps happening -- the tool measures
whether a CALL appears, not whether the behaviour is present.

**The `CheckDamage` line (3 calls, both `ItemModule` among them).** This port registers none. DK
decides whether the carry survives a hit by asking whether the damage came from the fighter he is
holding and running the reaction frame through `DamageTransactorImpl::calc_reaction_frame`. A new
status line, not a missing call.

**`WorkModule::set_int` in `ThrowF*`** and **`MotionModule::change_motion_inherit_frame` /
`WorkModule::is_flag` in SpecialLw** -- the last three, unread.

## Third pass: the two that needed the binary rather than the decompilation

Both of the last items turned out to be blocked on the same thing -- Ghidra's output is lossy in
exactly the places that decide behaviour -- and both were settled by going to the instructions.

### `CheckDamage` on the carry: the operator Ghidra will not name

Ghidra collapses every `lib::L2CValue` comparison operator into a single `operator_`, so DK's
shared carry `check_dmg` reads

    lib::L2CValue::L2CValue(aLStack64,_DAMAGE_LEVEL_FLY);
    uVar5 = lib::L2CValue::operator_(aLStack96,aLStack64);
    if ((uVar5 & 1) == 0) { ...drop the item...; return false; } else { return true; }

and `operator_` there is equally consistent with `==`, `!=`, `<` and `<=`. Each reading inverts
the move: the item and the carry break on weak hits instead of launching ones. ADR 0005 forbids
guessing, so the line stayed unimplemented until the PLT stub's MANGLED name settled it:

    0x7100004508  ldr w1, [x22, #0x1274]        ; ~lua_const DAMAGE_LEVEL_FLY
    0x710000451c  bl _ZNK3lib8L2CValueltERKS0_  ; operator<
    0x710000452c  tbz w21, #0x0, $+0x14         ; NOT less -> the ItemModule path
    0x7100004530  mov w1, #0x1                  ; less     -> return true

`lt`, so the item drops when the damage level IS `DAMAGE_LEVEL_FLY`. The mangled suffixes worth
keeping: `eqERKS0_` `==`, `neERKS0_` `!=`, `ltERKS0_` `<`, `leERKS0_` `<=`, `ixEN3phx6Hash40E`
`operator[](Hash40)`, `cvbEv` `operator bool`.

Two of the fields it reads are not in ParamLabels.csv and never will be -- they are C++ MEMBER
names exposed into L2C, which is what `tools/luaconst --hashdict` is for:
`0x10d723eebb owner_object_id_`, `0xdc56470f9 reaction_max_`. The trailing underscore is the
convention, and recognising it is what makes the rest of the `DamageInfo` table readable.

`DamageTransactorImpl::calc_reaction_frame` is CALLED, not retyped: it is exported from `main` at
0x404ec0 and imported by `lua2cpp_donkey.nro` through a PLT stub, so both calls land on the same
code. That matters more than convenience -- it multiplies by two global params (`0x10eb42a0a3`,
`0x10e33b9cd2`) that are in no label list this project has, so a reimplementation could not have
been checked.

Shipped on all ten of DK's carry statuses. `check-callparity.sh` for shoulder: 0 missing on both
groups, total 22 -> 20.

### `SPECIAL_LW_TURN`: the transition is in a function the decompilation cannot see

`lua2cpp_gaogaen.nro` never requests either retaliation status. `--scan 0xb574` and `0xb578` find
each constant at its three registration sites and at one comparison inside the shared movement
helper `FUN_7100013e50`, and nowhere else. Both transitions are engine-side -- and the function
that makes them has no entry in `vendor/decomp/main` at all, because `--callees` collects `bl`
targets and this one is only reached through a vtable. The corpus has a hole from 0xab86f0 to
0xab9800 for exactly that reason.

**A byte scan for the instruction found it anyway.** A status id in `main` is an immediate, and
`mov wN, #imm16` is `MOVZ`: `0x52800000 | (imm16 << 5) | Rd`. Scanning the 60 MB `.text` for the
two encodings gave 172 hits, two of them in the Gaogaen region, and the branch between them is
the whole answer:

    0xab9f14  cmp w0, #0x1df                  FIGHTER_STATUS_KIND_SPECIAL_LW
    0xab9f1c  [x23+0x50] vtable[0x108](0x2100000d)
                                              WorkModule::is_flag(FIGHTER_GAOGAEN_STATUS_
                                              SPECIAL_LW_FLAG_STANCE) -- the counter window
    0xab9f3c  ldr s0, [x20, #0x4]  fcmp / b.le  damage_info["damage_add_"] > 0.0
    0xab9f50  ldr s8, [x20, #0x160]           damage_info["damage_lr_"]
    0xab9f58  [x23+0x38] vtable[0xb0]()       PostureModule::lr
    0xab9f60  fmul s0, s8, s0
    0xab9f70  b.pl                            product >= 0 -> 0x1eb SPECIAL_LW_HIT
    0xab9f74  mov w1, #0x1ec                  product <  0 -> 0x1ec SPECIAL_LW_TURN
    0xab9fec  [x23+0x40] vtable[0x48](w1, 0)  StatusModule::change_status_request(.., false)

Every name there is resolved rather than guessed, and the technique is the one worth reusing: a
`lua_bind` wrapper is a one-line thunk, so its decompiled body IS the module offset and vtable
index. `PostureModule__lr_impl` is literally `(**(code **)(**(long **)(param_1 + 0x38) + 0xb0))()`
and `StatusModule__change_status_request_impl` is `[param_1 + 0x40] + 0x48`. The two `damage_info`
offsets come from `DamageInfo__store_l2c_table_impl`, which writes offset 0x4 under
`0xb9ea8580a damage_add_` and offset 0x160 under `0xa66ec16de damage_lr_`.

So the criterion is the attack's own left/right against the fighter's facing: hit from the front
he swings forward, hit from behind he turns and swings the other way, which is what the
`REVERSE_LR` in `game_speciallwturn` is for. That script and its two animations had been shipping
since the counter was ported with nothing able to reach them.

**Also found on the way, and fixed:** the three `pre` handlers are identical except for one log
flag. The stance passes `ACTION_TRIGGER_ON`; `SpecialLwHit_pre` and `SpecialLwTurn_pre` do not.
This port called the stance's `pre` for the retaliation, so it was filing an engine-triggered
action in the game's action log as a player-triggered one.

**And a deviation now recorded rather than implied:** only `SpecialLw_end` reverts the three
absorb calls. `SpecialLwHit_end` and `SpecialLwTurn_end` are literally `return 0`. The port
registers one `end` on all three because its PHASE and WINDOW_OPEN bookkeeping has to be cleared
when the retaliation ends -- a thing Incineroar has no equivalent of, because his motion choice
comes from the status and not from a variable. On the native path the extra call writes values
that are already set.

### What the port still does its own way here, deliberately

The engine's branch above is also the counter's TRIGGER, and the port's trigger is the damage
delta in `pikachu_revenge_frame`. That is left alone: the delta was confirmed working by playtest,
and moving the trigger into a `CheckDamage` line would mean deciding whether `damage_add_` is the
raw figure or the 0.4x-scaled one, which nothing read so far answers. So the new `CheckDamage`
line on SPECIAL_LW records the sign of `damage_lr_ * lr` and nothing else.

## Found while doing that, and NOT yet fixed: `revenge_kinetics` is not the helper it says it is

`FUN_7100013e50` is the movement setup all three of Incineroar's Revenge statuses call, and
`speciallw::revenge_kinetics` claims to be a transcription of it. Reading it properly for the TURN
status showed it is not. `disasm --trace 0x7100013e50` gives 50 calls; the port makes nine.

Three divergences, in increasing order of how much they could matter:

**1. The kinetic type is wrong, and this one is read off the instructions.** The port does
`change_kinetic(FIGHTER_KINETIC_TYPE_UNIQ)` unconditionally. The helper branches on the situation:

    0x7100014030  ldr w1, [x24, #0x58]    ; ~lua_const SITUATION_KIND_GROUND
    0x7100014048  bl _ZNK3lib8L2CValueeqERKS0_          globalTable[0x16] == GROUND ?
    0x710001405c  ldr w1, [x24, #0x410]   ; FIGHTER_KINETIC_TYPE_GROUND_STOP
    0x7100014078  bl KineticModule__change_kinetic_impl
    0x7100014080  ldr w1, [x24, #0x430]   ; FIGHTER_KINETIC_TYPE_FALL
    0x710001409c  bl KineticModule__change_kinetic_impl

`UNIQ` is what the three `pre` handlers pass to `init_settings`, which the port also does
correctly -- so the constant is right in one place and was reused in the wrong one.

**2. It is a THREE-WAY on the status, not one shape for all three.** The body is
`if (status == SPECIAL_LW) { A } else if (status == SPECIAL_LW_HIT || status == SPECIAL_LW_TURN)
{ B } else { return }`, each half then splitting again on ground/air. The status constants are the
three at 0x71000140a8 (`#0xd0` SPECIAL_LW), 0x7100014148 (`#0xb574` SPECIAL_LW_HIT) and
0x7100014174 (`#0xb578` SPECIAL_LW_TURN) -- the same two comparison sites that proved the
transitions are engine-side. The port has one path parameterised only by `air`.

**3. The stance hands the engine a MULTIPLIER where the port computes a speed.** The stance half
ends in `sv_kinetic_energy::set_speed_mul` on MOTION, with a literal `0xbf800000` (-1.0) in one
branch; the port always calls `set_speed`, with `sx * special_lw_start_x_mul` worked out in Rust.
Those are different operations on a stateful energy, and a -1.0 multiplier is exactly the shape of
"reverse what you came in with", which is what a status called TURN would want.

Also unverified rather than wrong: the port's second `clear_unable_energy` passes
FIGHTER_KINETIC_ENERGY_ID_CONTROL_NO_STOP on the stated reasoning that it is "CONTROL and its
no-stop twin, which is the pair that survives a hitstop". That is inference. The helper calls
`clear_unable_energy` three times and the trace's hints for them are GRAVITY, GRAVITY and CONTROL,
and all three take their id from `globalTable[5]` rather than a constant, so none of them has been
read yet.

**Why this is a separate round and not part of the TURN change.** The same doc comment that is
wrong about the kinetic type is right about the thing it was written for -- "allows the player to
slide around rather than killing their momentum" was a real playtest report and the MOTION-instead
-of-CONTROL fix is what cured it. Changing one constant inside a function whose other two thirds
are unread is how that regression comes back. The helper gets transcribed whole, against the
decompiled static in `vendor/decomp/statics/gaogaen.c:1`, or not at all.
