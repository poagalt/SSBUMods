// Incineroar's Alolan Whip (side-B): a travelling command grab that Irish-whips the victim into
// the wrestling ring ropes and hits them on the rebound.
//
// WHAT THE MOVE ACTUALLY IS, checked against SmashWiki rather than guessed. Incineroar dashes
// forward with both arms out. On a grab, a set of ropes rises up behind him at about the distance
// the dash covered, he whips the opponent into them, and they rebound back towards him. What
// happens next is decided by WHETHER AND WHEN an attack or special button is pressed:
//
//   pressed early, any time after the grab      -> BACK BODY DROP, launches them vertically
//   pressed just as they collide with him       -> LARIAT, launches them horizontally
//   never pressed                               -> FAILURE, both take 4%
//
// This replaced an earlier reading in which the follow-up was chosen from the control STICK and
// the whiffed lunge was the thing that dealt the 4%. Both were wrong, and the mapping is not a
// guess any more: it falls straight out of Incineroar's own params, below.
//
// TUNING FROM fighter/gaogaen/param/vl.prc, param_special_s[0], read with tools/prc-tool prcdump:
//   special_s_start_speed_mul                 1.3    the lunge, as a multiplier on MOTION energy
//   special_s_catch_speed_x_mul               0.2    momentum kept once the grab connects
//   special_s_catch_brake_x                   0.25
//   special_s_catch_clatter_frame             85     how long the victim can mash before escaping
//   special_s_through_target_speed            11.2   how fast the victim is whipped at the ropes
//   special_s_through_target_return_speed     2.8    how fast they come back
//   special_s_through_target_cancel_frame     30     frames the outbound leg lasts
//   special_s_through_target_speed_no_rope    1.6    the same throw with no ropes to bounce off
//   special_s_fail_damage                     4      what a failed follow-up costs him
//   special_s_lariat_offset_x1/x2            -24     LARIAT detection region, behind him
//   special_s_lariat_end_offset_x1/x2        -10     the tight, late part of that region
//   special_s_failure_offset_x1/x2            -6     where the rebound counts as a collision
//
// Those last three are the whole follow-up mapping in numbers: three nested regions behind
// Incineroar at 24, 10 and 6 units, matched by his FLAG_LARIAT_AREA, FLAG_LARIAT_END_AREA and
// FLAG_FAILURE_AREA. Which region means which is settled by one more number:
// game_specialslariat writes special_s_lariat_hit_frame = 9 and puts its arml hitbox out on frame
// 9. A victim returning at through_target_return_speed 2.8 covers the 24 units to Incineroar in
// 24 / 2.8 = 8.6 frames. So the 24-unit region is exactly "a lariat started now connects", and 10
// is where it is too late to start one. That is the "pressed just as they collide" window, and it
// is 24 units wide rather than a single frame.
//
// So: press with the victim between 10 and 24 units out -> LARIAT. Press earlier than that
// (including while they are still being held, or still flying outward) -> BACK BODY DROP. No
// press by the time they reach 6 units -> FAILURE. The outcome is decided by measured distance,
// not by an invented input scheme.
//
// FIVE STATUSES, NOT ONE WITH A PHASE VARIABLE. Incineroar has SPECIAL_S_THROW, _SHOULDER,
// _LARIAT, _FAILURE and _END as real status kinds (his own extras, 0x1E1-0x1E5). The earlier port
// crammed all of them into FIGHTER_STATUS_KIND_SPECIAL_S with a phase variable and re-entered its
// own Main function recursively, because adding status kinds looked impossible. It is not --
// smashline extends the status table, see ../status.rs -- so each follow-up now gets its own
// status, which is both the source's shape and the reason the recursive re-entry is gone.
//
// THE ROPES, AND WHY THE TRIP IS DRIVEN HERE RATHER THAN BY THE ENGINE.
//
// The visible ropes are Incineroar's ROPE article (FIGHTER_GAOGAEN_GENERATE_ARTICLE_ROPE,
// WEAPON_KIND_GAOGAEN_ROPE 0x1BA) and are not reproduced. Cloning an article is the machinery that
// crashes Ptooie on fighter load, and the documented fix for that crash -- merging `param_spikeball`
// into Pikachu's vl.prc -- is now known to be a cause rather than a cure: shipping ANY modified
// vl.prc null-dereferences in the same smashline function, `load_fighter_effects`. So that route is
// closed until the crash is understood, not merely worked around.
//
// The victim's statuses ARE all engine-global (SWING_GAOGAEN_THROWN 0x177, _ATTACH_ROPE 0x178,
// _RETURN 0x179) and Pikachu ships `gaogaen_special_s_thrown_big` for being on the receiving end.
// But those statuses read their trajectory off the GRABBER's `param_special_s` -- and Pikachu's is
// Skull Bash's, with no `special_s_through_target_speed` in it, so every lookup returns 0 and the
// whipped opponent goes nowhere. Replacing that param object is exactly what crashed on load.
//
// So the trajectory is driven from here instead, with Incineroar's own numbers, applied to the
// victim's posture directly. That is more faithful than it sounds: the values are his, the frame
// counts are his, and driving position rather than velocity means the arc does not vary with the
// victim's weight or their own physics -- which is what a scripted throw should do.
//
// What is genuinely lost: the ropes are invisible, and `_speed_over_rope` (the case where the
// victim clears the ropes entirely) is not modelled.
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

// param_special_s[0]. Kept as named constants rather than read at runtime because Pikachu's
// vl.prc has no param_special_s object to read them out of.
const START_SPEED_MUL: f32 = 1.3;
const CATCH_CLATTER_FRAME: i32 = 85;
const FAIL_DAMAGE: f32 = 4.0;
const THROUGH_CANCEL_FRAME: i32 = 30;
/// `special_s_through_target_speed` -- how fast the victim is whipped away.
const THROUGH_SPEED: f32 = 11.2;
/// `special_s_through_target_return_speed` -- how fast they come back off the ropes.
const THROUGH_RETURN_SPEED: f32 = 2.8;
/// Far edge of the LARIAT window, `special_s_lariat_offset_x`. A lariat started with the victim
/// here connects: 24 units at return_speed 2.8 is 8.6 frames, and the lariat's hitbox comes out on
/// frame 9.
const LARIAT_X: f32 = 24.0;
/// Near edge, `special_s_lariat_end_offset_x`. Inside this it is too late to start one.
const LARIAT_END_X: f32 = 10.0;
/// Frames after the whip before the lariat band is believed. The outbound leg runs at
/// through_target_speed 11.2, so the victim clears 24 units in about two frames; without this the
/// band would match on the way out as well as on the way back.
const RETURN_GUARD_FRAMES: i32 = 3;
/// `special_s_failure_offset_x` -- the rebound has arrived.
const FAILURE_X: f32 = 6.0;

/// Real frames into game_specialsthrow at which FLAG_THROW_ROPE fires. The ACMD raises it on
/// script frame 37, and FT_MOTION_RATE runs the script at 0.75 from frame 1 and 0.5 from frame 20
/// -- rate being game frames per motion frame -- so it lands about 24 real frames in. Used only as
/// a backstop; the flag itself is what this reads, now that deforeign keeps it.
const THROW_ROPE_FALLBACK_FRAME: i32 = 24;

/// The victim, captured when the grab connects. `CatchModule::capture_object_id` stops answering
/// once they leave the CATCHED status, and the follow-up decision needs their position for the
/// whole rebound, so it is cached rather than re-read.
static mut VICTIM: [u32; 8] = [0; 8];
/// Frames the victim has been held, against CATCH_CLATTER_FRAME.
static mut HELD_FOR: [i32; 8] = [0; 8];
/// Frames since FLAG_THROW_ROPE. -1 while the whip has not happened yet.
static mut SINCE_THROW: [i32; 8] = [-1; 8];

unsafe fn entry_of(boma: *mut BattleObjectModuleAccessor) -> usize {
    let e = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if e >= 8 { 0 } else { e }
}

unsafe fn reset(boma: *mut BattleObjectModuleAccessor) {
    let e = entry_of(boma);
    VICTIM[e] = *BATTLE_OBJECT_ID_INVALID as u32;
    HELD_FOR[e] = 0;
    SINCE_THROW[e] = -1;
}

/// The cached victim's module accessor, or None if they are gone.
unsafe fn victim_boma(entry: usize) -> Option<*mut BattleObjectModuleAccessor> {
    let id = VICTIM[entry];
    if id == *BATTLE_OBJECT_ID_INVALID as u32 || sv_battle_object::is_null(id) {
        return None;
    }
    let vboma = sv_battle_object::module_accessor(id);
    if vboma.is_null() {
        return None;
    }
    Some(vboma)
}

/// Horizontal distance from Pikachu to the cached victim, or None if there is no live victim.
unsafe fn victim_gap(boma: *mut BattleObjectModuleAccessor) -> Option<f32> {
    let vboma = victim_boma(entry_of(boma))?;
    Some((PostureModule::pos_x(vboma) - PostureModule::pos_x(boma)).abs())
}

/// Moves the victim one frame along the whip-and-rebound arc.
///
/// `frames_since` counts from FLAG_THROW_ROPE. Out at `through_target_speed` for
/// `through_target_cancel_frame` frames, then back at `through_target_return_speed` -- all three
/// numbers Incineroar's own. Direction is away from Pikachu's facing on the way out and toward him
/// coming back, which is what puts them in the lariat band roughly nine frames before contact.
///
/// Position rather than velocity on purpose: a scripted throw should trace the same arc regardless
/// of how heavy the victim is or what their own kinetic state was doing.
unsafe fn drive_victim(boma: *mut BattleObjectModuleAccessor, frames_since: i32) {
    let entry = entry_of(boma);
    let Some(vboma) = victim_boma(entry) else { return };

    let lr = PostureModule::lr(boma);
    let outbound = frames_since <= THROUGH_CANCEL_FRAME;
    let step = if outbound { THROUGH_SPEED } else { -THROUGH_RETURN_SPEED };

    // Their own physics would fight this every frame, so clear it first. The victim is in a
    // SWING_GAOGAEN status, which is a scripted capture -- they are not meant to be self-propelled.
    KineticModule::clear_speed_all(vboma);
    PostureModule::add_pos(vboma, &Vector3f { x: step * lr, y: 0.0, z: 0.0 });
}

/// An attack or special press this frame. SmashWiki says "an attack button", and in practice both
/// A and B commit the follow-up, so both are accepted.
unsafe fn followup_pressed(boma: *mut BattleObjectModuleAccessor) -> bool {
    ControlModule::check_button_trigger(boma, *CONTROL_PAD_BUTTON_ATTACK)
        || ControlModule::check_button_trigger(boma, *CONTROL_PAD_BUTTON_SPECIAL)
}

// ---------------------------------------------------------------------------------------------
// Shared status scaffolding. Every one of the six statuses wants the same init_settings and
// set_fighter_status_data, differing only in whether it starts stationary, so they share these.
// ---------------------------------------------------------------------------------------------

unsafe fn common_pre(fighter: &mut L2CFighterCommon, kinetic: i32) {
    StatusModule::init_settings(
        fighter.module_accessor,
        smash::app::SituationKind(*SITUATION_KIND_NONE),
        kinetic,
        GROUND_CORRECT_KIND_KEEP.into(),
        smash::app::GroundCliffCheckKind(*GROUND_CLIFF_CHECK_KIND_NONE),
        true,
        FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLAG.into(),
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_INT,
        FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLOAT.into(),
        0
    );
    FighterStatusModuleImpl::set_fighter_status_data(
        fighter.module_accessor,
        false,
        *FIGHTER_TREADED_KIND_NO_REAC,
        false,
        false,
        false,
        (*FIGHTER_LOG_MASK_FLAG_ACTION_TRIGGER_ON | *FIGHTER_LOG_MASK_FLAG_ATTACK_KIND_SPECIAL_S) as u64,
        0,
        FIGHTER_POWER_UP_ATTACK_BIT_SPECIAL_S.into(),
        0
    );
}

/// AIR OR GROUND, per Incineroar. He has a complete airborne Alolan Whip -- six motions, six ACMDs
/// and its own params (special_air_s_catch_accel_y 0.01, _speed_y_max 0.05: he drifts slowly down
/// while holding) -- and this port played the GROUNDED animation and grounded hitboxes for an
/// airborne side-B until his motion list was read instead of assumed.
///
/// The two names differ only by the `air` infix on every one of the six, so one helper covers all
/// of them and each status just names its own suffix.
unsafe fn situated(fighter: &mut L2CFighterCommon, suffix: &str) -> String {
    if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
        format!("special_s{suffix}")
    } else {
        format!("special_air_s{suffix}")
    }
}

/// Ground/air housekeeping shared by the follow-ups, which are all performed standing still.
unsafe fn hold_still(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    if fighter.global_table[0x16] != *SITUATION_KIND_GROUND {
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_AIR_STOP);
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
    } else {
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_GROUND_STOP);
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
    }
}

/// Hand control back once a follow-up's animation is done.
unsafe fn finish(fighter: &mut L2CFighterCommon) -> L2CValue {
    if MotionModule::is_end(fighter.module_accessor) {
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_END.into(), false.into());
        return 1.into();
    }
    0.into()
}

// ---------------------------------------------------------------------------------------------
// SPECIAL_S -- the lunge.
// ---------------------------------------------------------------------------------------------

unsafe extern "C" fn pikachu_specials_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    // FIGHTER_KINETIC_TYPE_MOTION, not UNIQ. The lunge is root motion out of
    // d01specialsstart.nuanmb -- 34.93 units of it in the Trans track -- scaled by
    // special_s_start_speed_mul. The old port used UNIQ and then immediately changed to
    // GROUND_STOP in Main, which pinned him in place: "it also no longer travels anywhere".
    common_pre(fighter, *FIGHTER_KINETIC_TYPE_MOTION);
    0.into()
}

unsafe extern "C" fn pikachu_specials_init(fighter: &mut L2CFighterCommon) -> L2CValue {
    // special_s_start_speed_mul = 1.3, applied to the motion energy exactly as G&W's up-B applies
    // his own 1.1 in ../specialhi.
    fighter.agent.clear_lua_stack();
    fighter.agent.push_lua_stack(&mut L2CValue::new_int(*FIGHTER_KINETIC_ENERGY_ID_MOTION as u64));
    fighter.agent.push_lua_stack(&mut L2CValue::new_num(START_SPEED_MUL));
    sv_kinetic_energy::set_speed_mul(fighter.lua_state_agent);
    0.into()
}

unsafe extern "C" fn pikachu_specials_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    reset(boma);
    MotionModule::change_motion(
        boma, Hash40::new(&situated(fighter, "_start")), 0.0, 1.0, false, 0.0, false, false
    );
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_specials_main_loop as *const () as _))
}

unsafe extern "C" fn pikachu_specials_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);

    // CatchModule::is_catch reports whether our CATCH collision has someone. The ACMD opens it on
    // frame 18 and clears it on 35, so this can only be true inside that window.
    if CatchModule::is_catch(boma) {
        VICTIM[entry] = CatchModule::capture_object_id(boma, false) as u32;
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_THROW.into(), false.into());
        return 1.into();
    }

    if MotionModule::is_end(boma) {
        // A whiffed lunge just ends. The 4% is special_s_fail_damage and belongs to the FAILURE
        // follow-up -- the price of not pressing anything after a successful grab -- not to
        // missing the grab in the first place.
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_END.into(), false.into());
        return 1.into();
    }
    0.into()
}

unsafe extern "C" fn pikachu_specials_end(fighter: &mut L2CFighterCommon) -> L2CValue {
    let _ = fighter;
    0.into()
}

// ---------------------------------------------------------------------------------------------
// SPECIAL_S_THROW -- the Irish whip, and the window in which the follow-up is chosen.
// ---------------------------------------------------------------------------------------------

unsafe extern "C" fn pikachu_specialsthrow_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    common_pre(fighter, *FIGHTER_KINETIC_TYPE_UNIQ);
    0.into()
}

unsafe extern "C" fn pikachu_specialsthrow_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    HELD_FOR[entry_of(boma)] = 0;
    SINCE_THROW[entry_of(boma)] = -1;
    MotionModule::change_motion(
        boma, Hash40::new(&situated(fighter, "_throw")), 0.0, 1.0, false, 0.0, false, false
    );
    hold_still(fighter);
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_specialsthrow_main_loop as *const () as _))
}

unsafe extern "C" fn pikachu_specialsthrow_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);

    // THE WHIP. game_specialsthrow raises FLAG_THROW_ROPE on its frame 37; deforeign keeps that
    // id now (see tools/reach/src/bin/deforeign.rs KEEP_STATUS_IDS) so it can be read here rather
    // than approximated. The frame fallback covers the case where the flag never arrives.
    if SINCE_THROW[entry] < 0 {
        HELD_FOR[entry] += 1;
        let thrown = WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_THROW_ROPE)
            || HELD_FOR[entry] >= THROW_ROPE_FALLBACK_FRAME;
        if thrown {
            SINCE_THROW[entry] = 0;
            // Hand the victim to the engine's own swing sequence. All of SWING_GAOGAEN_THROWN,
            // _ATTACH_ROPE and _RETURN are common statuses, so the outbound leg, the bounce and
            // the return are the game's code, not ours.
            let id = VICTIM[entry];
            if id != *BATTLE_OBJECT_ID_INVALID as u32 && !sv_battle_object::is_null(id) {
                let vboma = sv_battle_object::module_accessor(id);
                if !vboma.is_null() {
                    StatusModule::change_status_request_from_script(
                        vboma, *FIGHTER_STATUS_KIND_SWING_GAOGAEN_THROWN, false
                    );
                }
            }
        } else if HELD_FOR[entry] >= CATCH_CLATTER_FRAME {
            // special_s_catch_clatter_frame: the victim mashed out before the whip.
            fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_END.into(), false.into());
            return 1.into();
        }
        return 0.into();
    }

    SINCE_THROW[entry] += 1;
    // Carry the victim out to the ropes and back. Without this the engine's own SWING_GAOGAEN
    // statuses would try to, and they read numbers Pikachu's vl.prc does not have.
    drive_victim(boma, SINCE_THROW[entry]);

    // THE FOLLOW-UP DECISION, off the three param regions.
    let gap = victim_gap(boma);
    if followup_pressed(boma) {
        let in_lariat_band = SINCE_THROW[entry] >= RETURN_GUARD_FRAMES
            && gap.map_or(false, |g| g <= LARIAT_X && g >= LARIAT_END_X);
        let next = if in_lariat_band {
            RAT_STATUS_KIND_SPECIAL_S_LARIAT
        } else {
            RAT_STATUS_KIND_SPECIAL_S_SHOULDER
        };
        fighter.change_status(next.into(), false.into());
        return 1.into();
    }

    // No press, and the rebound has arrived. special_s_through_target_cancel_frame gates this so
    // the check cannot fire while the victim is still being held at zero distance.
    if SINCE_THROW[entry] > THROUGH_CANCEL_FRAME {
        let arrived = gap.map_or(true, |g| g <= FAILURE_X);
        if arrived || MotionModule::is_end(boma) {
            fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_FAILURE.into(), false.into());
            return 1.into();
        }
    }

    if MotionModule::is_end(boma) {
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_FAILURE.into(), false.into());
        return 1.into();
    }
    0.into()
}

// ---------------------------------------------------------------------------------------------
// The three follow-ups. Each is one motion plus its ACMD, which carries the whole hitbox and the
// ATTACK_ABS that releases the victim -- so the status code has nothing to do but play it.
// ---------------------------------------------------------------------------------------------

unsafe extern "C" fn pikachu_specialsshoulder_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    common_pre(fighter, *FIGHTER_KINETIC_TYPE_UNIQ);
    0.into()
}

unsafe extern "C" fn pikachu_specialsshoulder_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    MotionModule::change_motion(
        fighter.module_accessor, Hash40::new(&situated(fighter, "_shoulder")), 0.0, 1.0, false, 0.0, false, false
    );
    hold_still(fighter);
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_followup_loop as *const () as _))
}

unsafe extern "C" fn pikachu_specialslariat_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    MotionModule::change_motion(
        fighter.module_accessor, Hash40::new(&situated(fighter, "_lariat")), 0.0, 1.0, false, 0.0, false, false
    );
    hold_still(fighter);
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_followup_loop as *const () as _))
}

unsafe extern "C" fn pikachu_specialsfailure_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    // special_s_fail_damage = 4. Both sides take it; the victim's share is the ATTACK in
    // game_specialsfailure, this is his.
    DamageModule::add_damage(boma, FAIL_DAMAGE, 0);
    MotionModule::change_motion(
        boma, Hash40::new(&situated(fighter, "_failure")), 0.0, 1.0, false, 0.0, false, false
    );
    hold_still(fighter);
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_followup_loop as *const () as _))
}

unsafe extern "C" fn pikachu_followup_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    finish(fighter)
}

// ---------------------------------------------------------------------------------------------
// SPECIAL_S_END -- the recovery every path funnels into.
// ---------------------------------------------------------------------------------------------

unsafe extern "C" fn pikachu_specialsend_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    common_pre(fighter, *FIGHTER_KINETIC_TYPE_UNIQ);
    0.into()
}

unsafe extern "C" fn pikachu_specialsend_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    reset(boma);
    MotionModule::change_motion(
        boma, Hash40::new(&situated(fighter, "_end")), 0.0, 1.0, false, 0.0, false, false
    );
    hold_still(fighter);
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_specialsend_main_loop as *const () as _))
}

unsafe extern "C" fn pikachu_specialsend_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    if MotionModule::is_end(boma) {
        if StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND {
            fighter.change_status(FIGHTER_STATUS_KIND_WAIT.into(), false.into());
        } else {
            fighter.change_status(FIGHTER_STATUS_KIND_FALL.into(), false.into());
        }
        return 1.into();
    }
    0.into()
}

pub fn install() {
    Agent::new("pikachu")
        .status(Pre,  *FIGHTER_STATUS_KIND_SPECIAL_S, pikachu_specials_pre)
        .status(Init, *FIGHTER_STATUS_KIND_SPECIAL_S, pikachu_specials_init)
        .status(Main, *FIGHTER_STATUS_KIND_SPECIAL_S, pikachu_specials_main)
        .status(End,  *FIGHTER_STATUS_KIND_SPECIAL_S, pikachu_specials_end)
        .status(Pre,  RAT_STATUS_KIND_SPECIAL_S_THROW, pikachu_specialsthrow_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_THROW, pikachu_specialsthrow_main)
        .status(Pre,  RAT_STATUS_KIND_SPECIAL_S_SHOULDER, pikachu_specialsshoulder_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_SHOULDER, pikachu_specialsshoulder_main)
        .status(Pre,  RAT_STATUS_KIND_SPECIAL_S_LARIAT, pikachu_specialsshoulder_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_LARIAT, pikachu_specialslariat_main)
        .status(Pre,  RAT_STATUS_KIND_SPECIAL_S_FAILURE, pikachu_specialsshoulder_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_FAILURE, pikachu_specialsfailure_main)
        .status(Pre,  RAT_STATUS_KIND_SPECIAL_S_END, pikachu_specialsend_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_END, pikachu_specialsend_main)
        .install();
}
