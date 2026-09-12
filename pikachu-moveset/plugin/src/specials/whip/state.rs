// The work Incineroar's side-B keeps, kept somewhere Pikachu can keep it.
//
// THIS MODULE IS THE DECISION ADR 0006 SAYS WAS NEVER MADE, and the absence of which is why seven
// rounds of the old implementation drifted. A transcription of Incineroar's status functions has
// to read and write the work ids they read and write. Some of those cannot be used on a Pikachu,
// and the reason is not that they are out of range -- it is worse than that.
//
//     FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLAG_INVALID_SPECIAL_AIR_S             0x200000E7
//     FIGHTER_PIKACHU_INSTANCE_WORK_ID_FLAG_PAUSE                             0x200000E7
//
// The same index. Every fighter's instance work begins after the common block --
// FIGHTER_INSTANCE_WORK_ID_FLAG_TERM is 0x200000E0 -- and each then numbers its OWN ids from
// there, so gaogaen's eighth flag and Pikachu's eighth flag are the same slot. Writing
// Incineroar's flag on a Pikachu does not fail and does not corrupt anything outside the array:
// it silently sets FLAG_PAUSE. BACKLOG.md already lists "the FLAG_PAUSE leak" as one of two
// suspects for the Training Modpack save-state crash, and this is where it comes from.
//
// The same collision applies to
// FIGHTER_GAOGAEN_INSTANCE_WORK_ID_INT_BATTLE_OBJECT_ID_SWING_THROWN_FIGHTER at 0x100000C0,
// which lands inside Pikachu's own int block (his INT_TERM is 0x100000C2).
//
// THIS IS NOT A NEW DISCOVERY, AND THAT IS THE POINT. `src/specials/speciallw/mod.rs` has known it
// since Revenge was written:
//
//     STATE IS PER-ENTRY STATICS, NOT WORK VARIABLES. Incineroar's own work ids cannot be reused:
//     FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLAG_IS_ATTACK_HIT_REVENGE is 0x200000E3, which on Pikachu
//     is FLAG_MTRANS_SMPL_MOTION_END. [...] it is the same reason the ported d-air was writing
//     Pikachu's FLAG_PAUSE.
//
// One module worked it out, paid for it, and wrote it down -- and it stayed a note in one file
// instead of becoming a rule, so the whip and the carry went on to make the same class of mistake
// in their own ways. ADR 0006 promotes it to a decision and this module is where it lives. It is
// also the clearest argument for keeping Revenge rather than scrapping it: it is the one that got
// this right.
//
// SO THE PORT KEEPS ITS OWN, HERE, AND NOWHERE ELSE. One slot per constant, named after the
// constant it stands in for, so a transcribed line reads against the source line it came from:
//
//     source:  WorkModule::set_int(boma, BATTLE_OBJECT_ID_INVALID,
//                  FIGHTER_GAOGAEN_INSTANCE_WORK_ID_INT_BATTLE_OBJECT_ID_SWING_THROWN_FIGHTER)
//     port:    state::set_swing_thrown_fighter(entry, *BATTLE_OBJECT_ID_INVALID as u32)
//
// WHAT DOES **NOT** BELONG HERE. Status work (`FIGHTER_GAOGAEN_STATUS_SPECIAL_S_*`, the
// `0x21xxxxxx` and `0x1xxxxxx` families) is a different space: it is allocated per status from the
// common pool and its indices are small, so those ids are used directly through `WorkModule` the
// way the source does. Only INSTANCE work is shimmed, and only because of the collision above.
//
// Indexed by entry, because that is how everything else in this plugin is indexed and because a
// carry and a whip can be happening to two different Pikachus at once in a four-player game.
use smash::lib::lua_const::*;

const MAX: usize = 8;

/// `FIGHTER_GAOGAEN_INSTANCE_WORK_ID_INT_BATTLE_OBJECT_ID_SWING_THROWN_FIGHTER`.
///
/// The id of the fighter this whip is holding. Incineroar clears it to BATTLE_OBJECT_ID_INVALID
/// on the first frame of SPECIAL_S and reads it back in the throw, which is the only bookkeeping
/// of the victim his status tree keeps -- everything else about them is reached through the
/// capture link.
static mut SWING_THROWN_FIGHTER: [u32; MAX] = [0x5000_0000; MAX];

/// `FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLAG_INVALID_SPECIAL_AIR_S`.
///
/// Set on entry to SPECIAL_S and read to decide whether the airborne version may be used again
/// before landing -- the whip's once-per-airtime rule.
static mut INVALID_SPECIAL_AIR_S: [bool; MAX] = [false; MAX];

/// `FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLOAT_SPECIAL_S_LARIAT_AREA_HIT_POS_X`,
/// `_LARIAT_BUTTON_PUSH_POS_X` and `_LARIAT_AREA_LENGTH`.
///
/// The three floats the lariat's motion rate is computed from. These ones really are past the end
/// of Pikachu's float array -- gaogaen uses indices 0x50, 0x51 and 0x52 and
/// FIGHTER_PIKACHU_INSTANCE_WORK_ID_FLOAT_TERM is 0x4C -- so unlike the two above they would be a
/// genuine out-of-bounds write rather than a collision.
static mut LARIAT_AREA_HIT_POS_X: [f32; MAX] = [0.0; MAX];
static mut LARIAT_BUTTON_PUSH_POS_X: [f32; MAX] = [0.0; MAX];
static mut LARIAT_AREA_LENGTH: [f32; MAX] = [0.0; MAX];

/// Clears every slot, for the frame a whip begins.
///
/// Incineroar does not need this: his work lives in an array the engine zeroes when the fighter is
/// created and his statuses set what they need on entry. A `static mut` here outlives the match,
/// so a whip that ended in a previous stock would otherwise be visible to the next one.
pub fn reset(entry: usize) {
    if entry >= MAX {
        return;
    }
    unsafe {
        SWING_THROWN_FIGHTER[entry] = *BATTLE_OBJECT_ID_INVALID as u32;
        INVALID_SPECIAL_AIR_S[entry] = false;
        LARIAT_AREA_HIT_POS_X[entry] = 0.0;
        LARIAT_BUTTON_PUSH_POS_X[entry] = 0.0;
        LARIAT_AREA_LENGTH[entry] = 0.0;
    }
}

macro_rules! slot {
    ($get:ident, $set:ident, $store:ident, $ty:ty, $default:expr) => {
        pub fn $get(entry: usize) -> $ty {
            unsafe {
                if entry < MAX {
                    $store[entry]
                } else {
                    $default
                }
            }
        }
        pub fn $set(entry: usize, value: $ty) {
            unsafe {
                if entry < MAX {
                    $store[entry] = value;
                }
            }
        }
    };
}

slot!(
    swing_thrown_fighter,
    set_swing_thrown_fighter,
    SWING_THROWN_FIGHTER,
    u32,
    0x5000_0000
);
slot!(
    invalid_special_air_s,
    set_invalid_special_air_s,
    INVALID_SPECIAL_AIR_S,
    bool,
    false
);
slot!(
    lariat_area_hit_pos_x,
    set_lariat_area_hit_pos_x,
    LARIAT_AREA_HIT_POS_X,
    f32,
    0.0
);
slot!(
    lariat_button_push_pos_x,
    set_lariat_button_push_pos_x,
    LARIAT_BUTTON_PUSH_POS_X,
    f32,
    0.0
);
slot!(
    lariat_area_length,
    set_lariat_area_length,
    LARIAT_AREA_LENGTH,
    f32,
    0.0
);
