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

unsafe extern "C" fn pikachu_specialhi_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    StatusModule::init_settings(
        fighter.module_accessor, 
        smash::app::SituationKind(*SITUATION_KIND_NONE),  
        *FIGHTER_KINETIC_TYPE_MOTION_AIR_ANGLE,
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
        (*FIGHTER_LOG_MASK_FLAG_ATTACK_KIND_SPECIAL_HI | *FIGHTER_LOG_MASK_FLAG_ACTION_CATEGORY_ATTACK | *FIGHTER_LOG_MASK_FLAG_ACTION_TRIGGER_ON) as u64, 
        0, 
        FIGHTER_POWER_UP_ATTACK_BIT_SPECIAL_HI.into(), 
        0
    );
    0.into()
}

unsafe extern "C" fn pikachu_specialhi_init(fighter: &mut L2CFighterCommon) -> L2CValue {
    fighter.agent.clear_lua_stack();
    fighter.agent.push_lua_stack(&mut L2CValue::new_int(*FIGHTER_KINETIC_ENERGY_ID_MOTION as u64));
    fighter.agent.push_lua_stack(&mut L2CValue::new_num(1.1));
    sv_kinetic_energy::set_speed_mul(fighter.lua_state_agent);
    
    0.into()
}

unsafe extern "C" fn pikachu_specialhi_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    WorkModule::on_flag(fighter.module_accessor, *FIGHTER_GAMEWATCH_INSTANCE_WORK_ID_FLAG_SPECIAL_HI);    
    
    if fighter.global_table[0x16] != *SITUATION_KIND_GROUND {
        MotionModule::change_motion(fighter.module_accessor, Hash40::new("special_air_hi"), 0.0, 1.0, false, 0.0, false, false);
    } else {
        MotionModule::change_motion(fighter.module_accessor, Hash40::new("special_hi"), 0.0, 1.0, false, 0.0, false, false);
        fighter.set_situation(SITUATION_KIND_AIR.into());
        GroundModule::correct(fighter.module_accessor, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
    }
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_specialhi_main_loop as *const () as _))
}

unsafe extern "C" fn pikachu_specialhi_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    if !fighter.sub_transition_group_check_air_cliff().get_bool() {
        let is_end = MotionModule::is_end(fighter.module_accessor);
        if !is_end {
            if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
                let is_landing_enable = WorkModule::is_flag(fighter.module_accessor, *FIGHTER_GAMEWATCH_STATUS_SPECIAL_HI_FLAG_LANDING_ENABLE);
                if is_landing_enable {
                    fighter.change_status(FIGHTER_STATUS_KIND_LANDING_FALL_SPECIAL.into(), false.into());
                    return 1.into();
                }
            }
            return 0.into();
        }
        let attack_air_kind = ControlModule::get_attack_air_kind(fighter.module_accessor);
        WorkModule::set_int(fighter.module_accessor, attack_air_kind, *FIGHTER_GAMEWATCH_STATUS_SPECIAL_HI_WORK_INT_ATTACK_AIR_KIND);
        
        fighter.change_status(FIGHTER_DIDDY_STATUS_KIND_SPECIAL_HI_CHARGE_DAMAGE.into(), false.into());
    }
    return 1.into();
}

unsafe extern "C" fn pikachu_specialhi_exec(fighter: &mut L2CFighterCommon) -> L2CValue {
    let is_decide_direction = WorkModule::is_flag(fighter.module_accessor, *FIGHTER_GAMEWATCH_STATUS_SPECIAL_HI_FLAG_DECIDE_DIRECTION);
    if is_decide_direction {
        let stick_min = 0.25;
        let stick_x = ControlModule::get_stick_x(fighter.module_accessor).abs();
        
        if stick_min <= stick_x {
            PostureModule::set_rot(fighter.module_accessor, &Vector3f{x: 0.0, y: 0.0, z: 0.0}, 0);
            
            PostureModule::set_stick_lr(fighter.module_accessor, 0.0);
            PostureModule::update_rot_y_lr(fighter.module_accessor);
        }
        
        WorkModule::off_flag(fighter.module_accessor, *FIGHTER_GAMEWATCH_STATUS_SPECIAL_HI_FLAG_DECIDE_DIRECTION);
    }
    0.into()
}

unsafe extern "C" fn pikachu_specialhi_end(fighter: &mut L2CFighterCommon) -> L2CValue {
    if fighter.global_table[0xb] == *FIGHTER_STATUS_KIND_LANDING_FALL_SPECIAL {
        WorkModule::set_float(fighter.module_accessor, 40.0, *FIGHTER_INSTANCE_WORK_ID_FLOAT_LANDING_FRAME);
    }
    
    if fighter.global_table[0xb] != *FIGHTER_STATUS_KIND_LANDING {
        if fighter.global_table[0xb] != *FIGHTER_STATUS_KIND_LANDING_FALL_SPECIAL {
            return 0.into();
        }
    }
    
    WorkModule::off_flag(fighter.module_accessor, *FIGHTER_GAMEWATCH_INSTANCE_WORK_ID_FLAG_SPECIAL_HI);
    0.into()
}

pub fn install() {
    Agent::new("pikachu")
        .status(Pre, *FIGHTER_STATUS_KIND_SPECIAL_HI, pikachu_specialhi_pre)
        .status(Init, *FIGHTER_STATUS_KIND_SPECIAL_HI, pikachu_specialhi_init)
        .status(Main, *FIGHTER_STATUS_KIND_SPECIAL_HI, pikachu_specialhi_main)
        .status(Exec, *FIGHTER_STATUS_KIND_SPECIAL_HI, pikachu_specialhi_exec)
        .status(End, *FIGHTER_STATUS_KIND_SPECIAL_HI, pikachu_specialhi_end)
        .install();
}
