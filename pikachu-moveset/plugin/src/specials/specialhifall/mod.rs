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

unsafe extern "C" fn pikachu_specialhifall_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    StatusModule::init_settings(
        fighter.module_accessor, 
        smash::app::SituationKind(*SITUATION_KIND_AIR),  
        *FIGHTER_KINETIC_TYPE_UNIQ,
        GROUND_CORRECT_KIND_AIR.into(), 
        smash::app::GroundCliffCheckKind(*GROUND_CLIFF_CHECK_KIND_ON_DROP_BOTH_SIDES), 
        true, 
        FIGHTER_STATUS_WORK_KEEP_FLAG_ALL_FLAG.into(), 
        *FIGHTER_STATUS_WORK_KEEP_FLAG_ALL_INT, 
        FIGHTER_STATUS_WORK_KEEP_FLAG_ALL_FLOAT.into(), 
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
        FIGHTER_STATUS_ATTR_INTO_DOOR.into(), 
        FIGHTER_POWER_UP_ATTACK_BIT_SPECIAL_HI.into(), 
        0
    );
    0.into()
}

unsafe extern "C" fn pikachu_specialhifall_init(fighter: &mut L2CFighterCommon) -> L2CValue {
    KineticModule::change_kinetic(fighter.module_accessor, *FIGHTER_KINETIC_TYPE_FALL);
    let air_speed_y_stable = WorkModule::get_param_float(fighter.module_accessor, 0x12ec5626fe, 0);
    let speed_y_final = air_speed_y_stable * 0.5;
    
    fighter.agent.clear_lua_stack();
    fighter.agent.push_lua_stack(&mut L2CValue::new_int(*FIGHTER_KINETIC_ENERGY_ID_GRAVITY as u64));
    fighter.agent.push_lua_stack(&mut L2CValue::new_num(speed_y_final));
    sv_kinetic_energy::set_limit_speed(fighter.lua_state_agent);
    
    fighter.agent.clear_lua_stack();
    fighter.agent.push_lua_stack(&mut L2CValue::new_int(*FIGHTER_KINETIC_ENERGY_ID_GRAVITY as u64));
    fighter.agent.push_lua_stack(&mut L2CValue::new_num(speed_y_final));
    sv_kinetic_energy::set_stable_speed(fighter.lua_state_agent);
    
    0.into()
}

unsafe extern "C" fn pikachu_specialhifall_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let attack_air_kind = WorkModule::get_int(fighter.module_accessor, *FIGHTER_GAMEWATCH_STATUS_SPECIAL_HI_WORK_INT_ATTACK_AIR_KIND);
    ControlModule::set_attack_air_kind(fighter.module_accessor, attack_air_kind);
    WorkModule::enable_transition_term_group(fighter.module_accessor, *FIGHTER_STATUS_TRANSITION_GROUP_CHK_AIR_SPECIAL);
    WorkModule::enable_transition_term_group(fighter.module_accessor, *FIGHTER_STATUS_TRANSITION_GROUP_CHK_AIR_ATTACK);
    MotionModule::change_motion(fighter.module_accessor, Hash40::new("special_hi_open"), 0.0, 1.0, false, 0.0, false, false);
    
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_specialhifall_main_loop as *const () as _))
}

unsafe extern "C" fn pikachu_specialhifall_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    if !fighter.sub_transition_group_check_air_cliff().get_bool() {
        
        if fighter.sub_wait_ground_check_common(false.into()).get_bool()
        || fighter.sub_air_check_fall_common().get_bool() {
            return 1.into();
        }
        
        let is_enable_cancel = CancelModule::is_enable_cancel(fighter.module_accessor);
        if !is_enable_cancel {
            if !fighter.sub_transition_group_check_air_special().get_bool() {
                if !fighter.sub_transition_group_check_air_attack().get_bool() {
                    if fighter.global_table[0x16] != *SITUATION_KIND_GROUND {
                        let squat_stick_y = WorkModule::get_param_float(fighter.module_accessor, hash40("common"), hash40("squat_stick_y"));
                        if fighter.global_table[0x1b].get_f32() <= squat_stick_y {
                            let stick_prev_y = ControlModule::get_stick_prev_y(fighter.module_accessor);
                            if squat_stick_y < stick_prev_y {
                                fighter.change_status(FIGHTER_PIKACHU_STATUS_KIND_SPECIAL_HI_WARP.into(), false.into());
                                return 1.into();
                            }
                        }
                        let jump_stick_y = WorkModule::get_param_float(fighter.module_accessor, hash40("common"), hash40("jump_stick_y"));
                        if jump_stick_y <= fighter.global_table[0x1b].get_f32() {
                            let stick_prev_y = ControlModule::get_stick_prev_y(fighter.module_accessor);
                            if stick_prev_y < jump_stick_y {
                                fighter.change_status(FIGHTER_PIKACHU_STATUS_KIND_SPECIAL_HI_WARP.into(), false.into());
                                return 1.into();
                            }
                        }
                        return 0.into();
                    }
                    fighter.change_status(FIGHTER_PIKACHU_STATUS_KIND_SPECIAL_HI_WARP.into(), false.into());
                    return 1.into();
                }
            }
        }
    }
    return 1.into();
}

unsafe extern "C" fn pikachu_specialhifall_end(fighter: &mut L2CFighterCommon) -> L2CValue {
    if fighter.global_table[0xb] != *FIGHTER_PIKACHU_STATUS_KIND_SPECIAL_HI_WARP {
        let jump_count_max = WorkModule::get_int(fighter.module_accessor, *FIGHTER_INSTANCE_WORK_ID_INT_JUMP_COUNT_MAX);
        WorkModule::set_int(fighter.module_accessor, jump_count_max, *FIGHTER_INSTANCE_WORK_ID_INT_JUMP_COUNT);
    }
    0.into()
}

pub fn install() {
    Agent::new("pikachu")
        .status(Pre, *FIGHTER_PIKACHU_STATUS_KIND_SPECIAL_HI_END, pikachu_specialhifall_pre)
        .status(Init, *FIGHTER_PIKACHU_STATUS_KIND_SPECIAL_HI_END, pikachu_specialhifall_init)
        .status(Main, *FIGHTER_PIKACHU_STATUS_KIND_SPECIAL_HI_END, pikachu_specialhifall_main)
        .status(End, *FIGHTER_PIKACHU_STATUS_KIND_SPECIAL_HI_END, pikachu_specialhifall_end)
        .install();
}
