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

/// One past the highest id above. Anything new goes here and this moves up.
pub const RAT_STATUS_KIND_NUM: i32 = 0x1F7;

// Two things that must stay true, checked at compile time rather than trusted:
//   * every id above Pikachu's own NUM, so none of them shadows one of his statuses;
//   * every id below RAT_STATUS_KIND_NUM, so the declared ceiling is real.
// 0x1F0 is FIGHTER_PIKACHU_STATUS_KIND_NUM. It is written literally because the game's constant
// is a runtime lazy static and cannot be used in a const context.
const _: () = {
    let first = RAT_STATUS_KIND_SPECIAL_HI_CLOSE;
    let last = RAT_STATUS_KIND_SPECIAL_S_FAILURE;
    assert!(first >= 0x1F0);
    assert!(last < RAT_STATUS_KIND_NUM);
};
