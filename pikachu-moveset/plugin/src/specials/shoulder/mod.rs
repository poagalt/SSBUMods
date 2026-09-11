// DK's cargo carry as REAL STATUSES, which is how he does it.
//
// WHY THIS REPLACES THE MOTION-SWAP CARRY IN ../cargo.
//
// The previous design kept Pikachu in his own WAIT / WALK / TURN and only overrode the motion to
// DK's `shoulder_*` pose, so that his locomotion, speeds and jump would be inherited for free.
// Tracing the real Donkey Kong killed that idea: he is never in WAIT or WALK while carrying.
//
//     CATCH_PULL(0x3a) -> 0x1EA on throw_f
//     0x1EA -> 0x1EB   on frame 15 of throw_f
//     0x1EB <-> 0x1EC  shoulder_wait / shoulder_walk_middle
//     0x1EB <-> 0x1ED  shoulder_wait / shoulder_turn
//
// and every one of the reported symptoms falls out of the difference:
//
//   "as soon as pikachu starts moving the carry animation ends" -- WALK re-picks its own motion
//   every frame, so it overwrote the pose immediately.
//
//   "he moves with unbounded speed" -- WALK runs on CONTROL kinetic energy, which kept
//   accelerating while the shoulder animation ALSO carried root motion. DK's SHOULDER_WALK is
//   FIGHTER_KINETIC_TYPE_MOTION_LOOP: the animation moves him and there is no control energy to
//   run away with.
//
//   "none of the throws actually work" -- the cargo throw ACMDs use
//   ATTACK_ABS(FIGHTER_ATTACK_ABSOLUTE_KIND_THROW), which reads
//   FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT. Only a throw STATUS fills that in, so changing
//   motion to `throw_f_f` played an animation and nothing else.
//
//   "mario gets stuck to pikachu's feet" -- nothing was positioning the victim at all.
//
// EVERY init_settings BELOW IS DK'S, read out of his decompiled `pre` handlers in
// vendor/ssbu-status/.../lua2cpp_donkey.nro/donkey/donkey/status/Shoulder*/pre.c.
//
// THE VICTIM IS DRIVEN BY HAND, not by SHOULDERED_DONKEY. Round 3 established that requesting
// those statuses is the leading suspect for the freeze -- their code runs on the victim and is
// written for a carrier that is Donkey Kong. So their position is set directly from Pikachu's
// shoulder joint each frame, which is what the whip module already does for its own victim and
// has never crashed. What is lost is their pose: they hang upright rather than sitting.
use super::status::*;
use {
    smash::{
        lua2cpp::*,
        phx::*,
        app::{sv_animcmd::*, lua_bind::*, *},
        lib::{lua_const::*, L2CValue, L2CAgent},
        hash40
    },
    smash_script::*,
    smashline::{*, Priority::*}
};

/// Frame of `throw_f` at which DK hands off from SHOULDER_START to SHOULDER_WAIT.
///
/// Measured from the real fighter rather than assumed: the trace shows
/// `0x1EA -> 0x1EB on frame 15 of throw_f (anim end 24)`. His ACMD raises
/// FIGHTER_DONKEY_STATUS_SHOULDER_START_FLAG_SHOULDER_WAIT and his C++ turns that into the
/// transition; we have his flag in the ported script but nothing reads it, so the frame is used.
const SHOULDER_WAIT_FRAME: f32 = 15.0;

/// Stick deflection needed to walk or to turn, from `common/walk_stick_x` and
/// `common/turn_stick_x` -- the same commons DK's ShoulderWait reads.
const WALK_STICK_X: &str = "walk_stick_x";
const TURN_STICK_X: &str = "turn_stick_x";

/// The carried fighter's battle object id, per entry.
///
/// THE ID FORMAT IS MEASURED, NOT GUESSED. Three features were blocked on turning an entry id
/// into a battle object id, and rather than guess, the telemetry logged what the engine itself
/// hands back during Pikachu's ordinary grab. The answer: `captured object id = 0x10000` while we
/// were entry 0 and the victim was entry 1. So a fighter's object id is `entry << 16`, not the
/// `0x5000000n` the invalid sentinel suggested. Kept here as the id directly, since that is what
/// `sv_battle_object` wants.
static mut VICTIM: [u32; 8] = [0; 8];
static mut CARRY_FRAMES: [i32; 8] = [0; 8];

unsafe fn entry_of(boma: *mut BattleObjectModuleAccessor) -> usize {
    let e = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if e >= 8 { 0 } else { e }
}

unsafe fn victim_boma(entry: usize) -> *mut BattleObjectModuleAccessor {
    let id = VICTIM[entry];
    if id == *BATTLE_OBJECT_ID_INVALID as u32 || sv_battle_object::is_null(id) {
        return std::ptr::null_mut();
    }
    sv_battle_object::module_accessor(id)
}

/// Put the carried fighter on Pikachu's shoulder, every frame.
///
/// Reads the joint position rather than offsetting from the fighter's origin, so the victim rides
/// the animation instead of floating at a fixed height -- which is the difference between being
/// carried and being attached to a point in space.
unsafe fn hold_victim(boma: *mut BattleObjectModuleAccessor, entry: usize) {
    let vboma = victim_boma(entry);
    if vboma.is_null() {
        return;
    }
    let mut pos = Vector3f { x: 0.0, y: 0.0, z: 0.0 };
    ModelModule::joint_global_position(boma, Hash40::new("shoulderl"), &mut pos, true);
    // A joint that did not resolve returns the origin; falling back to the fighter's own position
    // plus a height is better than dropping the victim through the floor.
    if pos.x == 0.0 && pos.y == 0.0 {
        pos.x = PostureModule::pos_x(boma);
        pos.y = PostureModule::pos_y(boma) + 8.0;
    }
    PostureModule::set_pos(vboma, &pos);
    // Their own momentum must not fight the placement, or they drift off the shoulder.
    KineticModule::clear_speed_all(vboma);
}

/// True while the victim is still ours to carry.
unsafe fn still_held(entry: usize) -> bool {
    let vboma = victim_boma(entry);
    if vboma.is_null() {
        return false;
    }
    let s = StatusModule::status_kind(vboma);
    s == *FIGHTER_STATUS_KIND_CAPTURE_PULLED
        || s == *FIGHTER_STATUS_KIND_CAPTURE_WAIT
        || s == *FIGHTER_STATUS_KIND_CAPTURE_CUT
        || s == *FIGHTER_STATUS_KIND_THROWN
}

/// Common `pre` for the grounded carry states. DK uses the same shape for all of them and varies
/// only the kinetic type and the ground correction.
unsafe fn shoulder_pre(
    fighter: &mut L2CFighterCommon,
    kinetic: i32,
    ground_correct: i32,
) -> L2CValue {
    StatusModule::init_settings(
        fighter.module_accessor,
        smash::app::SituationKind(*SITUATION_KIND_NONE),
        kinetic,
        ground_correct as u32,
        smash::app::GroundCliffCheckKind(*GROUND_CLIFF_CHECK_KIND_NONE),
        true,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLAG,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_INT,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLOAT,
        0,
    );
    FighterStatusModuleImpl::set_fighter_status_data(
        fighter.module_accessor,
        false,
        *FIGHTER_TREADED_KIND_NO_REAC,
        false,
        false,
        false,
        0,
        0,
        0,
        0,
    );
    0.into()
}

// ---- SHOULDER_START : the lift, out of throw_f ----------------------------------------------

unsafe extern "C" fn shoulder_start_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(fighter, *FIGHTER_KINETIC_TYPE_MOTION, *GROUND_CORRECT_KIND_GROUND_OTTOTTO)
}

unsafe extern "C" fn shoulder_start_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] = 0;
    crate::telemetry::probe::event(format!(
        "[rat] SHOULDER p{entry} START, victim={:#x}; {}",
        VICTIM[entry],
        crate::telemetry::probe::ctx(boma)
    ));
    // DK stays on `throw_f` through the lift; the pose only changes at SHOULDER_WAIT.
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_start_loop as *const () as _))
}

unsafe extern "C" fn shoulder_start_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    hold_victim(boma, entry);

    if !still_held(entry) {
        fighter.change_status(FIGHTER_STATUS_KIND_WAIT.into(), false.into());
        return 1.into();
    }
    if MotionModule::frame(boma) >= SHOULDER_WAIT_FRAME || MotionModule::is_end(boma) {
        fighter.change_status(RAT_STATUS_KIND_SHOULDER_WAIT.into(), false.into());
        return 1.into();
    }
    0.into()
}

// ---- SHOULDER_WAIT : the hold, and the decision point ----------------------------------------

unsafe extern "C" fn shoulder_wait_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(fighter, *FIGHTER_KINETIC_TYPE_MOTION, *GROUND_CORRECT_KIND_GROUND_OTTOTTO)
}

unsafe extern "C" fn shoulder_wait_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    MotionModule::change_motion(boma, Hash40::new("shoulder_wait"), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_wait_loop as *const () as _))
}

/// The throw the input asks for, if any.
///
/// DK'S OWN CHECK, READ OUT OF THE BINARY. `FUN_7100022520` is the helper every Shoulder loop
/// calls, and the public dump omits it because it could not name it. `tools/disasm` resolves the
/// NRO's PLT against its .dynsym, so it reads plainly now:
///
///   LinkModule::get_node_object_id      the fighter on his shoulder
///   ControlModule::get_clatter_time     on the VICTIM; `<= 0` returns early with no throw
///   globalTable[0x21] & <four consecutive constants>     cat2, four bits apart
///   L2CFighterCommon::IsThrowStick
///   globalTable[0x20] & <an OR of four more>             cat1
///   five change_status calls, chosen by a hash-indexed lookup
///
/// `globalTable[0x20]` is cat1 and `[0x21]` is cat2 -- ShoulderStart/main_loop.c and
/// SpecialNLoop/main_loop.c in the same dump index them with named CAT1 and CAT2 constants, which
/// is what fixes the two indices. The four consecutive cat2 constants can only be
/// FIGHTER_PAD_CMD_CAT2_FLAG_THROW_F, _B, _HI and _LW: adjacent slots in the constant table, and
/// bits 0x100000, 0x200000, 0x400000, 0x800000 -- the four throw commands, adjacent in the
/// enumeration too.
///
/// SO THE DIRECTION IS THE ENGINE'S, NOT A THRESHOLD OF OURS. This used to compare
/// `get_stick_x` and `get_stick_y` against 0.5. The cat2 throw commands are the same thing the
/// game resolves for an ordinary grab, which means smash-stick deflection, the input buffer and
/// the per-controller deadzones all behave here exactly as they do out of a normal grab -- none
/// of which a raw stick read reproduces.
///
/// The button path survives as the second half of DK's own test, `IsThrowStick` together with a
/// cat1 button mask, because that is what he does when the command flag has not fired. TRIGGER
/// rather than held: the grab button is still down when the carry begins, and reading it as held
/// is what committed a throw on the first carry frame and froze the game in round 2.
unsafe fn requested_throw(fighter: &mut L2CFighterCommon) -> Option<i32> {
    let boma = fighter.module_accessor;
    let cat2 = ControlModule::get_command_flag_cat(boma, 1);
    if cat2 & *FIGHTER_PAD_CMD_CAT2_FLAG_THROW_HI != 0 {
        return Some(RAT_STATUS_KIND_THROW_F_HI);
    }
    if cat2 & *FIGHTER_PAD_CMD_CAT2_FLAG_THROW_LW != 0 {
        return Some(RAT_STATUS_KIND_THROW_F_LW);
    }
    if cat2 & *FIGHTER_PAD_CMD_CAT2_FLAG_THROW_B != 0 {
        return Some(RAT_STATUS_KIND_THROW_F_B);
    }
    if cat2 & *FIGHTER_PAD_CMD_CAT2_FLAG_THROW_F != 0 {
        return Some(RAT_STATUS_KIND_THROW_F_F);
    }

    let pressed = ControlModule::check_button_trigger(boma, *CONTROL_PAD_BUTTON_ATTACK)
        || ControlModule::check_button_trigger(boma, *CONTROL_PAD_BUTTON_SPECIAL);
    if !pressed {
        return None;
    }
    // `IsThrowStick` DECIDES THE DIRECTION, IT DOES NOT GATE THE THROW. A cargo attack press with
    // the stick neutral is a forward throw in the real move, so requiring a deflection here would
    // remove the most common input in the move. DK's helper reads it alongside the cat1 button
    // mask for the same reason: to know whether a direction was asked for.
    let x = if fighter.IsThrowStick().get_bool() {
        ControlModule::get_stick_x(boma) * PostureModule::lr(boma)
    } else {
        0.0
    };
    let y = ControlModule::get_stick_y(boma);
    if y > 0.5 {
        Some(RAT_STATUS_KIND_THROW_F_HI)
    } else if y < -0.5 {
        Some(RAT_STATUS_KIND_THROW_F_LW)
    } else if x < -0.5 {
        Some(RAT_STATUS_KIND_THROW_F_B)
    } else {
        Some(RAT_STATUS_KIND_THROW_F_F)
    }
}

unsafe extern "C" fn shoulder_wait_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    hold_victim(boma, entry);

    if !still_held(entry) {
        crate::telemetry::probe::event(format!(
            "[rat] SHOULDER p{entry} victim lost after {}f; {}",
            CARRY_FRAMES[entry],
            crate::telemetry::probe::ctx(boma)
        ));
        fighter.change_status(FIGHTER_STATUS_KIND_WAIT.into(), false.into());
        return 1.into();
    }
    if let Some(next) = requested_throw(fighter) {
        crate::telemetry::probe::event(format!(
            "[rat] SHOULDER p{entry} throw {} after {}f; {}",
            crate::telemetry::probe::st(next),
            CARRY_FRAMES[entry],
            crate::telemetry::probe::ctx(boma)
        ));
        fighter.change_status(next.into(), false.into());
        return 1.into();
    }
    // Airborne -- walked off a ledge.
    if StatusModule::situation_kind(boma) != *SITUATION_KIND_GROUND {
        fighter.change_status(RAT_STATUS_KIND_SHOULDER_FALL.into(), false.into());
        return 1.into();
    }

    // DK's ShoulderWait reads `common/turn_stick_x` and `common/walk_stick_x` in that order, and
    // turn wins -- the stick pointing behind you is a turn, not a backwards walk.
    let stick_x = ControlModule::get_stick_x(boma);
    let lr = PostureModule::lr(boma);
    let turn_min = WorkModule::get_param_float(boma, smash::hash40(TURN_STICK_X), 0);
    let walk_min = WorkModule::get_param_float(boma, smash::hash40(WALK_STICK_X), 0);
    if stick_x * lr < -turn_min.max(0.1) {
        fighter.change_status(RAT_STATUS_KIND_SHOULDER_TURN.into(), false.into());
        return 1.into();
    }
    if stick_x.abs() > walk_min.max(0.1) {
        fighter.change_status(RAT_STATUS_KIND_SHOULDER_WALK.into(), false.into());
        return 1.into();
    }
    0.into()
}

// ---- SHOULDER_WALK ---------------------------------------------------------------------------

unsafe extern "C" fn shoulder_walk_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    // MOTION_LOOP: the animation moves him. This is the fix for the unbounded speed.
    shoulder_pre(fighter, *FIGHTER_KINETIC_TYPE_MOTION_LOOP, *GROUND_CORRECT_KIND_GROUND_OTTOTTO)
}

unsafe extern "C" fn shoulder_walk_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    // Three walk speeds, as DK has. Picked from stick deflection rather than from current speed,
    // because MOTION_LOOP means there is no speed until the motion supplies one.
    let x = ControlModule::get_stick_x(boma).abs();
    let pose = if x > 0.85 {
        "shoulder_walk_fast"
    } else if x > 0.5 {
        "shoulder_walk_middle"
    } else {
        "shoulder_walk_slow"
    };
    MotionModule::change_motion(boma, Hash40::new(pose), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_walk_loop as *const () as _))
}

unsafe extern "C" fn shoulder_walk_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    hold_victim(boma, entry);

    if !still_held(entry) {
        fighter.change_status(FIGHTER_STATUS_KIND_WAIT.into(), false.into());
        return 1.into();
    }
    if let Some(next) = requested_throw(fighter) {
        fighter.change_status(next.into(), false.into());
        return 1.into();
    }
    if StatusModule::situation_kind(boma) != *SITUATION_KIND_GROUND {
        fighter.change_status(RAT_STATUS_KIND_SHOULDER_FALL.into(), false.into());
        return 1.into();
    }
    let stick_x = ControlModule::get_stick_x(boma);
    let lr = PostureModule::lr(boma);
    if stick_x * lr < -0.1 {
        fighter.change_status(RAT_STATUS_KIND_SHOULDER_TURN.into(), false.into());
        return 1.into();
    }
    if stick_x.abs() <= 0.1 {
        fighter.change_status(RAT_STATUS_KIND_SHOULDER_WAIT.into(), false.into());
        return 1.into();
    }
    0.into()
}

// ---- SHOULDER_TURN ---------------------------------------------------------------------------

unsafe extern "C" fn shoulder_turn_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(fighter, *FIGHTER_KINETIC_TYPE_TURN, *GROUND_CORRECT_KIND_GROUND)
}

unsafe extern "C" fn shoulder_turn_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    MotionModule::change_motion(boma, Hash40::new("shoulder_turn"), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_turn_loop as *const () as _))
}

unsafe extern "C" fn shoulder_turn_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    hold_victim(boma, entry);

    if !still_held(entry) {
        fighter.change_status(FIGHTER_STATUS_KIND_WAIT.into(), false.into());
        return 1.into();
    }
    if MotionModule::is_end(boma) {
        // The turn is what flips him; doing it at the end matches DK, whose animation carries the
        // rotation and whose status only commits it once.
        PostureModule::reverse_lr(boma);
        PostureModule::update_rot_y_lr(boma);
        fighter.change_status(RAT_STATUS_KIND_SHOULDER_WAIT.into(), false.into());
        return 1.into();
    }
    0.into()
}

// ---- SHOULDER_FALL / _LANDING ----------------------------------------------------------------

unsafe extern "C" fn shoulder_fall_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(fighter, *FIGHTER_KINETIC_TYPE_MOTION_FALL, *GROUND_CORRECT_KIND_AIR)
}

unsafe extern "C" fn shoulder_fall_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    MotionModule::change_motion(boma, Hash40::new("shoulder_fall"), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_fall_loop as *const () as _))
}

unsafe extern "C" fn shoulder_fall_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    hold_victim(boma, entry);

    if !still_held(entry) {
        fighter.change_status(FIGHTER_STATUS_KIND_FALL.into(), false.into());
        return 1.into();
    }
    if let Some(next) = requested_throw(fighter) {
        fighter.change_status(next.into(), false.into());
        return 1.into();
    }
    if StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND {
        fighter.change_status(RAT_STATUS_KIND_SHOULDER_LANDING.into(), false.into());
        return 1.into();
    }
    0.into()
}

unsafe extern "C" fn shoulder_landing_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(fighter, *FIGHTER_KINETIC_TYPE_MOTION, *GROUND_CORRECT_KIND_GROUND_OTTOTTO)
}

unsafe extern "C" fn shoulder_landing_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    MotionModule::change_motion(boma, Hash40::new("shoulder_landing"), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_landing_loop as *const () as _))
}

unsafe extern "C" fn shoulder_landing_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    hold_victim(boma, entry);

    if !still_held(entry) {
        fighter.change_status(FIGHTER_STATUS_KIND_WAIT.into(), false.into());
        return 1.into();
    }
    if MotionModule::is_end(boma) {
        fighter.change_status(RAT_STATUS_KIND_SHOULDER_WAIT.into(), false.into());
        return 1.into();
    }
    0.into()
}

// ---- THE FOUR CARGO THROWS -------------------------------------------------------------------

/// Common `pre` for the throws, matching the engine's own THROW status shape.
unsafe extern "C" fn throw_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    StatusModule::init_settings(
        fighter.module_accessor,
        smash::app::SituationKind(*SITUATION_KIND_NONE),
        *FIGHTER_KINETIC_TYPE_MOTION,
        *GROUND_CORRECT_KIND_KEEP as u32,
        smash::app::GroundCliffCheckKind(*GROUND_CLIFF_CHECK_KIND_NONE),
        true,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLAG,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_INT,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLOAT,
        0,
    );
    FighterStatusModuleImpl::set_fighter_status_data(
        fighter.module_accessor,
        false,
        *FIGHTER_TREADED_KIND_NO_REAC,
        false,
        false,
        false,
        (*FIGHTER_LOG_MASK_FLAG_ACTION_CATEGORY_CATCH | *FIGHTER_LOG_MASK_FLAG_ACTION_TRIGGER_ON) as u64,
        0,
        0,
        0,
    );
    0.into()
}

/// Start one of the four throws.
///
/// THE TARGET WORK VARIABLES ARE THE POINT. The cargo throw ACMDs read
/// FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT, _TARGET_HIT_GROUP and _TARGET_HIT_NO and pass them
/// to ATK_HIT_ABS. Without them the ATTACK_ABS(THROW) has nobody to throw, which is precisely why
/// changing motion alone did nothing. Filling them here is what turns the animation into a throw.
unsafe fn throw_main(fighter: &mut L2CFighterCommon, motion: &str) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    WorkModule::set_int64(boma, VICTIM[entry] as u64, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT);
    WorkModule::set_int64(boma, 0, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_GROUP);
    WorkModule::set_int64(boma, 0, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_NO);
    crate::telemetry::probe::event(format!(
        "[rat] SHOULDER p{entry} throw motion '{motion}' target={:#x}; {}",
        VICTIM[entry],
        crate::telemetry::probe::ctx(boma)
    ));
    MotionModule::change_motion(boma, Hash40::new(motion), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(throw_loop as *const () as _))
}

unsafe extern "C" fn throw_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    if MotionModule::is_end(boma) {
        VICTIM[entry_of(boma)] = *BATTLE_OBJECT_ID_INVALID as u32;
        let ground = StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND;
        let next = if ground { *FIGHTER_STATUS_KIND_WAIT } else { *FIGHTER_STATUS_KIND_FALL };
        fighter.change_status(next.into(), false.into());
        return 1.into();
    }
    0.into()
}

unsafe extern "C" fn throw_ff_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    throw_main(fighter, "throw_f_f")
}
unsafe extern "C" fn throw_fb_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    throw_main(fighter, "throw_f_b")
}
unsafe extern "C" fn throw_fhi_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    throw_main(fighter, "throw_f_hi")
}
unsafe extern "C" fn throw_flw_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    throw_main(fighter, "throw_f_lw")
}

/// Called from ../cargo when the grab lands, to hand the carry over.
pub unsafe fn begin(fighter: &mut L2CFighterCommon, victim: u32) {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    VICTIM[entry] = victim;
    CARRY_FRAMES[entry] = 0;
    StatusModule::change_status_request_from_script(
        boma, RAT_STATUS_KIND_SHOULDER_START, false
    );
}

pub fn install() {
    Agent::new("pikachu")
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_START,   shoulder_start_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_START,   shoulder_start_main)
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_WAIT,    shoulder_wait_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_WAIT,    shoulder_wait_main)
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_WALK,    shoulder_walk_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_WALK,    shoulder_walk_main)
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_TURN,    shoulder_turn_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_TURN,    shoulder_turn_main)
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_FALL,    shoulder_fall_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_FALL,    shoulder_fall_main)
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_LANDING, shoulder_landing_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_LANDING, shoulder_landing_main)
        .status(Pre,  RAT_STATUS_KIND_THROW_F_F,  throw_pre)
        .status(Main, RAT_STATUS_KIND_THROW_F_F,  throw_ff_main)
        .status(Pre,  RAT_STATUS_KIND_THROW_F_B,  throw_pre)
        .status(Main, RAT_STATUS_KIND_THROW_F_B,  throw_fb_main)
        .status(Pre,  RAT_STATUS_KIND_THROW_F_HI, throw_pre)
        .status(Main, RAT_STATUS_KIND_THROW_F_HI, throw_fhi_main)
        .status(Pre,  RAT_STATUS_KIND_THROW_F_LW, throw_pre)
        .status(Main, RAT_STATUS_KIND_THROW_F_LW, throw_flw_main)
        .install();
}
