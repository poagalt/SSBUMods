// The three INSTANCE work slots Samus's Charge Shot keeps, kept somewhere Pikachu can keep them.
//
// SAME RULE, SAME REASON AS ../whip/state.rs, AND IT APPLIES HERE WITHOUT A SINGLE CHANGE. A
// fighter's instance work is numbered from the end of the common block, so every fighter's Nth
// slot is the SAME slot. Samus's three are:
//
//     FIGHTER_SAMUS_INSTANCE_WORK_ID_FLAG_ST_INIT            0x200000E1
//     FIGHTER_SAMUS_INSTANCE_WORK_ID_INT_EFH_CHARGE_MAX      0x100000BD
//     FIGHTER_SAMUS_INSTANCE_WORK_ID_INT_SPECIAL_N_COUNT     0x100000BE
//
// `FIGHTER_INSTANCE_WORK_ID_FLAG_TERM` is 0x200000E0 and `..._INT_TERM` is 0x100000B?, so all
// three land INSIDE Pikachu's own instance block -- his INT_TERM is 0x100000C2. Writing them
// would not fail and would not corrupt anything outside the array; it would silently set three of
// HIS variables, which is exactly the failure mode that put Incineroar's Revenge flag on
// Pikachu's FLAG_PAUSE and cost the whip three rounds.
//
// SO THE CHARGE LIVES HERE, and `SPECIAL_N_COUNT` is the important one: it is the STORED CHARGE,
// the number that survives firing, dying and respawning in the real move and the single piece of
// state the whole of Charge Shot is about. Putting it on a Pikachu work id would have made it
// share a slot with something of his, and the bug that produces -- a charge that resets or jumps
// when an unrelated system writes -- is the hardest kind to attribute from a playtest.
//
// Indexed by entry, like everything else in this plugin, because four Pikachus can charge at once.
const MAX: usize = 8;

/// `FIGHTER_SAMUS_INSTANCE_WORK_ID_INT_SPECIAL_N_COUNT` -- the stored charge, in frames.
///
/// Counts UP from 0 to `cshot_charge_frame` (112 in samus's `param_special_n`). The charge is
/// spent by `SpecialNF_init`, which copies it into the status work and zeroes this.
static mut COUNT: [i32; MAX] = [0; MAX];

/// `FIGHTER_SAMUS_INSTANCE_WORK_ID_INT_EFH_CHARGE_MAX` -- the effect handle for the max-charge
/// glow, so it can be removed again.
///
/// Kept even though this port cannot spawn that effect: `charge_max` is one of SAMUS's effects
/// and ADR 0002 keeps Pikachu's own effect archives, so nothing here ever fills this in. It
/// exists so the two teardown sites read as their source lines do, and so that turning the
/// effect on later is a one-line change rather than a re-derivation.
static mut EFH_CHARGE_MAX: [i32; MAX] = [0; MAX];

/// `FIGHTER_SAMUS_INSTANCE_WORK_ID_FLAG_ST_INIT` -- "this status was entered fresh", set by
/// `SpecialN_pre` and cleared by `SpecialN_init` once it has run the one-time setup.
static mut ST_INIT: [bool; MAX] = [false; MAX];

pub fn count(e: usize) -> i32 {
    if e >= MAX { return 0; }
    unsafe { COUNT[e] }
}
pub fn set_count(e: usize, v: i32) {
    if e < MAX { unsafe { COUNT[e] = v; } }
}
pub fn inc_count(e: usize) {
    if e < MAX { unsafe { COUNT[e] += 1; } }
}
pub fn efh_charge_max(e: usize) -> i32 {
    if e >= MAX { return 0; }
    unsafe { EFH_CHARGE_MAX[e] }
}
pub fn set_efh_charge_max(e: usize, v: i32) {
    if e < MAX { unsafe { EFH_CHARGE_MAX[e] = v; } }
}
pub fn st_init(e: usize) -> bool {
    if e >= MAX { return false; }
    unsafe { ST_INIT[e] }
}
pub fn set_st_init(e: usize, v: bool) {
    if e < MAX { unsafe { ST_INIT[e] = v; } }
}

/// Clear one fighter's charge.
///
/// NOT CALLED YET, AND THAT IS A STATED GAP. In the real move the stored charge is per-STOCK: it
/// survives being hit and dying, and it is reset when the match is. Nothing in the six statuses
/// resets it, because on Samus the number lives in HER instance work and the engine clears that
/// with the rest of the fighter on respawn. A static outlives that, so a Pikachu who charges,
/// dies and respawns keeps the charge here where the real move would have lost it.
///
/// Left as a function rather than wired to a guessed hook, because the right trigger is a fighter
/// reset and this project has not yet needed one -- `crate::telemetry` sees every frame and is
/// where it would go once a playtest says it matters.
pub fn reset(e: usize) {
    set_count(e, 0);
    set_efh_charge_max(e, 0);
    set_st_init(e, false);
}
