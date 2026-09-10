// Incineroar's Alolan Whip (side-B): a command grab with three follow-ups.
//
// WHAT THE ENGINE PROVIDES, AND WHY THIS IS FEASIBLE AT ALL
// The victim side is entirely global, not fighter-specific. `CATCH` in Incineroar's
// game_specialsstart names FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED as the caught fighter's
// status, and that whole family -- CATCHED, SHOULDER, LARIAT, THROWN, RETURN, FAILURE,
// ATTACH_ROPE -- is in the common status list. Pikachu already carries a
// `gaogaen_special_s_thrown_big` motion for being on the receiving end of it. So the capture,
// the clatter mash-out and the throw all run on engine code that works for any grabber, and the
// only part that needs writing is Pikachu's own side.
//
// The follow-ups then release the victim with ATTACK_ABS(FIGHTER_ATTACK_ABSOLUTE_KIND_THROW, ...),
// which is the standard throw path, again global.
//
// TUNING FROM fighter/gaogaen/param/vl.prc, param_special_s[0] (50 params, read with prcdump):
//   special_s_start_speed_mul       1.3    the lunge
//   special_s_catch_speed_x_mul     0.2    momentum kept once the grab connects
//   special_s_catch_brake_x         0.25
//   special_s_catch_clatter_frame   85     how long the victim can mash before breaking out
//   special_s_fail_damage           4      self-damage on a whiff
//   special_s_invalid_capture_frame 60     re-grab lockout
//   special_s_lariat_motion_rate_min/max 1 / 2
//
// ONE STATUS, PHASES. Incineroar has FIGHTER_GAOGAEN_STATUS_KIND_SPECIAL_S_SHOULDER, _LARIAT,
// _THROW, _END and _FAILURE; Pikachu has none of them, and adding status kinds is not needed --
// each follow-up is the same SPECIAL_S status playing a different motion. Same approach as
// Revenge in ../speciallw.
//
// THE ONE THING NOT RECOVERABLE FROM THE DATA is which input picks which follow-up. It is not in
// the ACMDs, not in the params, and the status code that reads it is C++. The mapping below is an
// interpretation, deliberately isolated in `chosen_followup` so it is a one-place change once
// playtesting settles it.
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

// From param_special_s[0].
const CATCH_CLATTER_FRAME: i32 = 85;
const FAIL_DAMAGE: f32 = 4.0;

const PHASE_START: i32 = 0;
const PHASE_SHOULDER: i32 = 1;
const PHASE_LARIAT: i32 = 2;
const PHASE_THROW: i32 = 3;
const PHASE_FAILURE: i32 = 4;

static mut PHASE: [i32; 8] = [PHASE_START; 8];
static mut HELD_FOR: [i32; 8] = [0; 8];
static mut DID_CATCH: [bool; 8] = [false; 8];

unsafe fn entry_of(boma: *mut BattleObjectModuleAccessor) -> usize {
    let e = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if e >= 8 { 0 } else { e }
}

fn motion_for(phase: i32) -> &'static str {
    match phase {
        PHASE_SHOULDER => "special_s_shoulder",
        PHASE_LARIAT => "special_s_lariat",
        PHASE_THROW => "special_s_throw",
        PHASE_FAILURE => "special_s_failure",
        _ => "special_s_start",
    }
}

// INTERPRETED, not recovered. Up gives the upward toss, down gives the spinning Lariat, and
// anything else gives the plain Throw. Change here and nowhere else.
unsafe fn chosen_followup(boma: *mut BattleObjectModuleAccessor) -> i32 {
    let y = ControlModule::get_stick_y(boma);
    if y > 0.5 {
        PHASE_SHOULDER
    } else if y < -0.5 {
        PHASE_LARIAT
    } else {
        PHASE_THROW
    }
}

unsafe extern "C" fn pikachu_specials_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    StatusModule::init_settings(
        fighter.module_accessor,
        smash::app::SituationKind(*SITUATION_KIND_NONE),
        *FIGHTER_KINETIC_TYPE_UNIQ,
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
    0.into()
}

unsafe extern "C" fn pikachu_specials_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);

    if PHASE[entry] == PHASE_START {
        HELD_FOR[entry] = 0;
        DID_CATCH[entry] = false;
    }
    MotionModule::change_motion(
        boma, Hash40::new(motion_for(PHASE[entry])), 0.0, 1.0, false, 0.0, false, false
    );

    if fighter.global_table[0x16] != *SITUATION_KIND_GROUND {
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_AIR_STOP);
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
    } else {
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_GROUND_STOP);
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
    }
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_specials_main_loop as *const () as _))
}

unsafe extern "C" fn pikachu_specials_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);

    if PHASE[entry] == PHASE_START {
        // CatchModule::is_catch reports whether our CATCH collision has someone. The ACMD opens
        // it on frame 18 and clears it on 35, so this is only ever true inside that window.
        if CatchModule::is_catch(boma) {
            DID_CATCH[entry] = true;
            HELD_FOR[entry] += 1;
            // The victim mashes out at clatter_frame; committing on the frame before means the
            // follow-up always beats the break-out rather than racing it.
            let stick_committed = ControlModule::get_stick_y(boma).abs() > 0.5;
            if HELD_FOR[entry] >= CATCH_CLATTER_FRAME - 1 || stick_committed {
                PHASE[entry] = chosen_followup(boma);
                return pikachu_specials_main(fighter);
            }
            return 0.into();
        }

        if MotionModule::is_end(boma) {
            // The lunge finished without catching anyone. Incineroar takes
            // special_s_fail_damage for that, which is the move's whole risk.
            if !DID_CATCH[entry] {
                DamageModule::add_damage(boma, FAIL_DAMAGE, 0);
                PHASE[entry] = PHASE_FAILURE;
                return pikachu_specials_main(fighter);
            }
        }
    }

    if MotionModule::is_end(boma) {
        PHASE[entry] = PHASE_START;
        HELD_FOR[entry] = 0;
        DID_CATCH[entry] = false;
        if StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND {
            fighter.change_status(FIGHTER_STATUS_KIND_WAIT.into(), false.into());
        } else {
            fighter.change_status(FIGHTER_STATUS_KIND_FALL.into(), false.into());
        }
    }
    0.into()
}

unsafe extern "C" fn pikachu_specials_end(fighter: &mut L2CFighterCommon) -> L2CValue {
    let entry = entry_of(fighter.module_accessor);
    PHASE[entry] = PHASE_START;
    HELD_FOR[entry] = 0;
    DID_CATCH[entry] = false;
    0.into()
}

pub fn install() {
    Agent::new("pikachu")
        .status(Pre,  *FIGHTER_STATUS_KIND_SPECIAL_S, pikachu_specials_pre)
        .status(Main, *FIGHTER_STATUS_KIND_SPECIAL_S, pikachu_specials_main)
        .status(End,  *FIGHTER_STATUS_KIND_SPECIAL_S, pikachu_specials_end)
        .install();
}
