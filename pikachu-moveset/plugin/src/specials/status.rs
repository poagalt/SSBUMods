// BRAND-NEW status kinds for this moveset, past the end of Pikachu's own table.
//
// WHY THIS IS ALLOWED. It looked for a long time as though a ported move could only ever reuse a
// status kind the host fighter already has, and that belief did real damage: it is why the up-B's
// two extra statuses were remapped onto SPECIAL_HI_WARP and SPECIAL_HI_END, and why Alolan Whip's
// follow-ups were crammed into one status with a phase variable and its command grab retargeted
// at the ordinary capture. The belief was wrong.
//
// smashline extends the table. Its set_status_scripts hook (vendor/sl_create_agent.rs:738 and
// :802) reads the fighter's current status total out of `global_table[0xC]`, installs every
// registered script with `sv_set_status_func`, tracks `max_new = max_new.max(status.id + 1)`, and
// writes that back to `global_table[0xC]`. A status id past the fighter's own NUM is not a
// mistake there -- it is the documented path, and it is how the status table grows.
//
// PIKACHU'S OWN TABLE, from skyline-smash's constant list:
//   0x1E1 SPECIAL_HI_WARP     Quick Attack's second leg
//   0x1E2 SPECIAL_HI_END
//   0x1E3 SPECIAL_S_HOLD      Skull Bash charge
//   0x1E4 SPECIAL_S_WEAK
//   0x1E5 SPECIAL_S_ATTACK
//   0x1E6 SPECIAL_S_END
//   0x1E7 SPECIAL_LW_HIT      Thunder
//   0x1E8..0x1EF              Final Smash
//   0x1F0 NUM
//
// So 0x1F0 upward is free, and nothing here collides with a Pikachu status.
//
// THE BUG THAT MADE THIS NECESSARY. Reusing 0x1E1 SPECIAL_HI_WARP for the parachute meant
// entering Pikachu's Quick Attack warp status. The up-B port only registers Pre/Main/End on it,
// so his own Init and Exec still ran -- and those are what hide his model, apply the warp
// displacement and spawn the direction guide. Playtest saw all three at once: "up b appears to
// teleport modified pikachu extremely high and/or turn him invisible" and "spawns quick attack
// arrows around if I change the direction of the stick while using it". A status you do not fully
// own is not yours; a new one is.
//
// tools/check-status.sh guards the range.

/// G&W's parachute-close status. George's mod used Diddy's own SPECIAL_HI_CHARGE for this, which
/// is 0x1F0 -- the same number, because Diddy simply has more statuses than Pikachu.
pub const RAT_STATUS_KIND_SPECIAL_HI_CLOSE: i32 = 0x1F0;
/// G&W's parachute-open / drift status. George used Diddy's SPECIAL_HI_CHARGE_DAMAGE = 0x1F1.
pub const RAT_STATUS_KIND_SPECIAL_HI_OPEN: i32 = 0x1F1;

// Alolan Whip's follow-ups, one status each, mirroring Incineroar's own
// FIGHTER_GAOGAEN_STATUS_KIND_SPECIAL_S_SHOULDER / _LARIAT / _THROW / _END / _FAILURE.
pub const RAT_STATUS_KIND_SPECIAL_S_SHOULDER: i32 = 0x1F2;
pub const RAT_STATUS_KIND_SPECIAL_S_LARIAT: i32 = 0x1F3;
pub const RAT_STATUS_KIND_SPECIAL_S_THROW: i32 = 0x1F4;
pub const RAT_STATUS_KIND_SPECIAL_S_END: i32 = 0x1F5;
pub const RAT_STATUS_KIND_SPECIAL_S_FAILURE: i32 = 0x1F6;


// DK's CARGO CARRY, which is eleven real statuses on his side and was a motion swap on ours.
//
// THE MOTION-SWAP DESIGN WAS THE BUG, and the trace of the real Donkey Kong says so directly:
//
//   CATCH_PULL(0x3a) -> 0x1EA on throw_f
//   0x1EA -> 0x1EB   on frame 15 of throw_f        SHOULDER_START -> SHOULDER_WAIT
//   0x1EB <-> 0x1EC  shoulder_wait / shoulder_walk_middle
//   0x1EB <-> 0x1ED  shoulder_wait / shoulder_turn
//
// He is never in WAIT or WALK while carrying. This port kept Pikachu in HIS OWN movement
// statuses and only overrode the motion, on the reasoning that it would inherit his locomotion
// for free. What it actually inherited was a status that re-picks its own motion every frame, so
// the carry pose was overwritten the instant he moved -- playtest: "as soon as pikachu starts
// moving the carry over shoulder animation ends" -- and a CONTROL kinetic energy that kept
// accelerating on top of the animation's own root motion, which is the "unbounded speed".
//
// The kinetic types come straight from DK's `pre` handlers and are the whole answer to the speed:
//
//   SHOULDER_START / WAIT / LANDING   FIGHTER_KINETIC_TYPE_MOTION
//   SHOULDER_WALK                     FIGHTER_KINETIC_TYPE_MOTION_LOOP
//   SHOULDER_TURN                     FIGHTER_KINETIC_TYPE_TURN
//   SHOULDER_FALL                     FIGHTER_KINETIC_TYPE_MOTION_FALL
//
// all with GROUND_CORRECT_KIND_GROUND_OTTOTTO (the teeter correction) except TURN (GROUND) and
// FALL (AIR), and GROUND_CLIFF_CHECK_KIND_NONE throughout. MOTION_LOOP means the ANIMATION moves
// him -- there is no control energy to run away with.
pub const RAT_STATUS_KIND_SHOULDER_START: i32 = 0x1F7;
pub const RAT_STATUS_KIND_SHOULDER_WAIT: i32 = 0x1F8;
pub const RAT_STATUS_KIND_SHOULDER_WALK: i32 = 0x1F9;
pub const RAT_STATUS_KIND_SHOULDER_TURN: i32 = 0x1FA;
pub const RAT_STATUS_KIND_SHOULDER_FALL: i32 = 0x1FB;
pub const RAT_STATUS_KIND_SHOULDER_LANDING: i32 = 0x1FC;

// The four cargo throws. DK has FIGHTER_DONKEY_STATUS_KIND_THROW_F_F / _B / _HI / _LW as real
// statuses, which is why changing motion to `throw_f_f` did nothing useful: the ACMD's
// ATTACK_ABS(THROW) reads FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT, and only a throw STATUS
// ever fills that in. Playtest: "none of the throws actually work after initiating cargo throw".
pub const RAT_STATUS_KIND_THROW_F_F: i32 = 0x1FD;
pub const RAT_STATUS_KIND_THROW_F_B: i32 = 0x1FE;
pub const RAT_STATUS_KIND_THROW_F_HI: i32 = 0x1FF;
pub const RAT_STATUS_KIND_THROW_F_LW: i32 = 0x200;

// NOT REPRODUCED, and named so the gap is visible: DK also has SHOULDER_JUMP_SQUAT,
// _JUMP_SQUAT_B, _JUMP and _PASS -- jumping while carrying, and dropping through a soft platform.
// Those need Pikachu's own jump params threaded through four more statuses, and the carry is
// worth having without them; the jump transition is suppressed instead, so the input does
// nothing rather than doing something wrong.

/// Revenge's RETALIATION, which Incineroar runs as its own status and we were trying to run as a
/// re-entry into SPECIAL_LW.
///
/// THAT RE-ENTRY IS WHY THE FIRE NEVER CAME OUT. The counter was detected, the boost was banked,
/// and then `change_status_request_from_script(SPECIAL_LW)` was issued from inside SPECIAL_LW --
/// a request to enter the status you are already in, which the engine drops. `PHASE_COUNTER` was
/// set so the next `main` WOULD have picked the `special_lw` attack motion, but no `main` ever
/// ran, so the stance animation carried on and the hitbox never existed.
///
/// Incineroar does not do this: `lua2cpp_gaogaen.nro` has `SpecialLwHit` as a real status with
/// its own pre, main, main_loop and end, identical in shape to SpecialLw -- same
/// FIGHTER_KINETIC_TYPE_UNIQ, same GROUND_CORRECT_KIND_KEEP, and the same FUN_7100013e50
/// movement setup -- differing only in which motion it plays.
pub const RAT_STATUS_KIND_SPECIAL_LW_HIT: i32 = 0x201;

/// One past the highest id above. Anything new goes here and this moves up.
pub const RAT_STATUS_KIND_NUM: i32 = 0x202;

// Two things that must stay true, checked at compile time rather than trusted:
//   * every id above Pikachu's own NUM, so none of them shadows one of his statuses;
//   * every id below RAT_STATUS_KIND_NUM, so the declared ceiling is real.
// 0x1F0 is FIGHTER_PIKACHU_STATUS_KIND_NUM. It is written literally because the game's constant
// is a runtime lazy static and cannot be used in a const context.
const _: () = {
    let first = RAT_STATUS_KIND_SPECIAL_HI_CLOSE;
    let last = RAT_STATUS_KIND_SPECIAL_LW_HIT;
    assert!(first >= 0x1F0);
    assert!(last < RAT_STATUS_KIND_NUM);
};
