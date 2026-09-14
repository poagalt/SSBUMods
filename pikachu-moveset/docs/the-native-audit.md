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

**`param_special_hi`: `stick_min`, `stick_mul`, `landing_frame`.** `src/params` provides Incineroar's
`param_special_s` and `param_special_lw` and **nothing for gamewatch at all**, so G&W's up-B reads
Pikachu's own `param_special_hi` -- Quick Attack's -- or zero. Resolved through
`vendor/refs/param-labels/ParamLabels.csv`, which is what that clone is for. Bounded work:
extend the provider the way it was extended for gaogaen.

**The `CheckDamage` line (3 calls, both `ItemModule` among them).** This port registers none. DK
decides whether the carry survives a hit by asking whether the damage came from the fighter he is
holding and running the reaction frame through `DamageTransactorImpl::calc_reaction_frame`. A new
status line, not a missing call.

**`WorkModule::set_int` in `ThrowF*`** and **`MotionModule::change_motion_inherit_frame` /
`WorkModule::is_flag` in SpecialLw** -- the last three, unread.
