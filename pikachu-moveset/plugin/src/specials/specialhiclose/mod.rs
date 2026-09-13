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

unsafe extern "C" fn pikachu_specialhiclose_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    StatusModule::init_settings(
        fighter.module_accessor, 
        smash::app::SituationKind(*SITUATION_KIND_NONE),  
        *FIGHTER_KINETIC_TYPE_UNIQ,
        GROUND_CORRECT_KIND_KEEP.into(), 
        smash::app::GroundCliffCheckKind(*GROUND_CLIFF_CHECK_KIND_ON_DROP_BOTH_SIDES), 
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
        (*FIGHTER_LOG_MASK_FLAG_ATTACK_KIND_SPECIAL_HI | *FIGHTER_LOG_MASK_FLAG_ACTION_CATEGORY_ATTACK) as u64, 
        0, 
        FIGHTER_POWER_UP_ATTACK_BIT_SPECIAL_HI.into(), 
        0
    );
    0.into()
}

unsafe extern "C" fn pikachu_specialhiclose_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    MotionModule::change_motion(fighter.module_accessor, Hash40::new("special_hi_close"), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_specialhiclose_main_loop as *const () as _))
}

unsafe extern "C" fn pikachu_specialhiclose_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    if fighter.sub_transition_group_check_air_cliff().get_bool() {
        return 1.into();
    }
    if fighter.sub_wait_ground_check_common(false.into()).get_bool()
    || fighter.sub_air_check_fall_common().get_bool() {
         return 1.into();
    }
    
    let is_enable_cancel = CancelModule::is_enable_cancel(fighter.module_accessor);
    if !is_enable_cancel {
        let is_end = MotionModule::is_end(fighter.module_accessor);
        if !is_end {
            return 0.into();
        } else {
            let status = if fighter.global_table[0x16] != *SITUATION_KIND_GROUND { *FIGHTER_STATUS_KIND_FALL } else { *FIGHTER_STATUS_KIND_WAIT };
            fighter.change_status(status.into(), false.into());
            return 1.into();
        }
    }
    return 0.into();
}

unsafe extern "C" fn pikachu_specialhiclose_end(fighter: &mut L2CFighterCommon) -> L2CValue {
    if fighter.global_table[0x16] == *SITUATION_KIND_AIR {
        let jump_count_max = WorkModule::get_int(fighter.module_accessor, *FIGHTER_INSTANCE_WORK_ID_INT_JUMP_COUNT_MAX);
        WorkModule::set_int(fighter.module_accessor, jump_count_max, *FIGHTER_INSTANCE_WORK_ID_INT_JUMP_COUNT);
    }
    0.into()
}

pub fn install() {
    Agent::new("pikachu")
        .status(Pre, RAT_STATUS_KIND_SPECIAL_HI_CLOSE, pikachu_specialhiclose_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_HI_CLOSE, pikachu_specialhiclose_main)
        .status(End, RAT_STATUS_KIND_SPECIAL_HI_CLOSE, pikachu_specialhiclose_end)
        // ALSO ON `Exit`, BECAUSE `End` DOES NOT FIRE FOR A STATUS PAST PIKACHU'S OWN COUNT.
        //
        // G&W's `SpecialHiClose_end` exhausts the jump counter, and this had been transcribed
        // correctly and never run: modified Pikachu kept his double jump after the up-B, which
        // G&W does not. The body is a plain assignment, so running on both lines is harmless and
        // needs no latch. See the note in `specials::whip` for the measurement and the two engine
        // offsets that put both lines inside one function.
        .status(Exit, RAT_STATUS_KIND_SPECIAL_HI_CLOSE, pikachu_specialhiclose_end)
        .install();
}
