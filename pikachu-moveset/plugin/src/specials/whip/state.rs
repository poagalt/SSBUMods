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


/// `FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLOAT_ROPE_X` (0x1000007), WHICH THE VICTIM READS OFF US.
///
/// Not a slot Pikachu could hold even if it were in range: it is STATUS-scoped, so its index is
/// into whatever float array our own custom status was given, and the reader is not us. The
/// victim's `status_SwingGaogaenCatched` resolves the captor through
/// `LinkModule::get_parent_id(LINK_NO_CAPTURE)` and calls `WorkModule::get_float` on the captor's
/// accessor with gaogaen's id. So it is kept here and `src/params` answers the read, exactly as it
/// already answers the twelve `param_special_s` reads from the same status.
///
/// WHY IT MATTERS: IT IS THE DIFFERENCE BETWEEN 11.2 AND 1.6. The victim picks its throw speed
/// from our `is_flag(GENERATED_ROPE)`:
///
///     rope    -> special_s_through_target_speed        11.2   + rope_x, + _speed_over_rope
///     no rope -> special_s_through_target_speed_no_rope 1.6
///
/// Answering "no rope" -- which is honest under ADR 0004 -- put the victim on a 1.6/frame path and
/// then the port hand-triggered a rebound at the ROPE's distance of -24, which a 1.6/frame throw
/// was never going to reach on the game's timing. "The travel distance/time isn't right" is that
/// mismatch, measured: +14.74 at f30, -10.29 at f40, -24.69 at f55.
static mut ROPE_X: [f32; MAX] = [0.0; MAX];
/// Whether `ROPE_X` currently holds a real coordinate, so `src/params` answers `GENERATED_ROPE`
/// honestly rather than claiming a rope exists before there is anywhere to put one.
static mut ROPE_X_SET: [bool; MAX] = [false; MAX];
/// NOT ONE OF INCINEROAR'S IDS -- a port-only latch, and the only slot in this file that is.
///
/// `whip_update_areas` needs to know whether the victim has reached the lariat area at least once.
/// The three areas sit BEHIND the captor (x of -24, -10 and -6) and the victim starts the throw
/// beside him at roughly 0, so "past the end line" and "past the failure line" are both TRUE on
/// the first frame. Ungated, FLAG_FAILURE_AREA would be raised immediately and the THROW loop --
/// which tests it before anything else -- would end the whole throw on frame one.
///
/// Incineroar needs no equivalent, which is why there is no constant to name this after: the rope
/// article raises his area flags from real collisions, so he never has to infer "on the way back"
/// from a coordinate. This latch belongs to the ADR 0004 substitution rather than to the
/// transcription, and it is named for what it does.
static mut LARIAT_BEEN_OUT: [bool; MAX] = [false; MAX];

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
        LARIAT_BEEN_OUT[entry] = false;
        LARIAT_AREA_HIT_POS_X[entry] = 0.0;
        LARIAT_BUTTON_PUSH_POS_X[entry] = 0.0;
        LARIAT_AREA_LENGTH[entry] = 0.0;
        ROPE_X[entry] = 0.0;
        ROPE_X_SET[entry] = false;
        FLIGHT_FRAMES[entry] = 0;
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
    lariat_been_out,
    set_lariat_been_out,
    LARIAT_BEEN_OUT,
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
slot!(rope_x, set_rope_x, ROPE_X, f32, 0.0);
slot!(rope_x_set, set_rope_x_set, ROPE_X_SET, bool, false);

/// Frames elapsed in the current leg of the victim's flight -- THROWN, then ATTACH_ROPE.
///
/// NOT ONE OF INCINEROAR'S IDS. For him the rope article ends each leg by sending an event; this
/// counts instead. See `whip_flight` for the two numbers and where each comes from.
static mut FLIGHT_FRAMES: [i32; MAX] = [0; MAX];
slot!(flight_frames, set_flight_frames, FLIGHT_FRAMES, i32, 0);
