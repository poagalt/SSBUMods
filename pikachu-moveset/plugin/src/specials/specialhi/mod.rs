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

// WHICH OF G&W'S UP-B PARAMS THIS PORT ACTUALLY MODELS, because tools/check-srcparams.sh asks and
// the answer should be written down rather than rediscovered.
//
// `gamewatch param_special_hi` has five: stick_min 0.25, stick_mul 30, limit_y_mul 0.5,
// landing_frame 40, pass_speed_mul 1.1. This port uses stick_min in _exec, landing_frame in _end,
// and pass_speed_mul as the motion speed multiplier below.
//
// stick_mul AND limit_y_mul ARE BOTH MODELLED NOW, and what unblocked them was finding Game &
// Watch's own decompiled status scripts -- see vendor/ssbu-status. They were listed as NOT
// MODELLED because a "30-unit multiplier" could not be interpreted from the name alone. It is 30
// DEGREES of launch-angle deviation, applied in _exec below; limit_y_mul halves his terminal fall
// speed while the parachute is out, in ../specialhifall.
unsafe extern "C" fn pikachu_specialhi_init(fighter: &mut L2CFighterCommon) -> L2CValue {
    fighter.agent.clear_lua_stack();
    fighter.agent.push_lua_stack(&mut L2CValue::new_int(*FIGHTER_KINETIC_ENERGY_ID_MOTION as u64));
    // gamewatch param_special_hi pass_speed_mul = 1.1
    fighter.agent.push_lua_stack(&mut L2CValue::new_num(1.1));
    sv_kinetic_energy::set_speed_mul(fighter.lua_state_agent);

    0.into()
}

unsafe extern "C" fn pikachu_specialhi_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    // G&W sets FIGHTER_GAMEWATCH_INSTANCE_WORK_ID_FLAG_SPECIAL_HI here, but that id is
    // 0x200000E2, which on Pikachu is FLAG_MTRANS_SMPL_GROUND -- so the port was corrupting his
    // motion-transition state on every up-B. Nothing in this port ever READS the flag (only
    // G&W's own C++ does), so the write is simply removed rather than remapped.

    // INSURANCE AGAINST A PERMANENTLY INVISIBLE PIKACHU.
    //
    // Pikachu's own Quick Attack hides his model: expression_specialairhi1, _2, _end and
    // expression_specialhiend all call VisibilityModule::set_whole, and the ONLY thing that turns
    // it back on is the tail of those same scripts. So any path that hides him and then leaves
    // early strands him invisible for the rest of the stock -- which is exactly what playtest saw
    // ("turn him invisible") while the up-B was wrongly routed through SPECIAL_HI_WARP.
    //
    // Those motions are unreachable now: the status remap is gone and ../lockout bounces out of
    // SPECIAL_HI_END before any motion change. This is belt to that braces, because the cost of
    // being wrong is a mod that looks completely broken and the cost of the call is nothing.
    VisibilityModule::set_whole(fighter.module_accessor, true);

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
        // G&W raises FLAG_PARACHUTE_OPEN on script frame 33 of game_specialhi, and that -- not
        // the end of the animation -- is when the parachute comes out. The port used
        // MotionModule::is_end as a stand-in because the flag had no source; ../acmd_specialhi
        // now sets it on his exact frame, so read the real thing and keep is_end only as the
        // backstop for a motion that somehow runs past it.
        let is_end = WorkModule::is_flag(fighter.module_accessor, *FIGHTER_GAMEWATCH_STATUS_SPECIAL_HI_FLAG_PARACHUTE_OPEN)
            || MotionModule::is_end(fighter.module_accessor);
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
        
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_HI_OPEN.into(), false.into());
    }
    return 1.into();
}

unsafe extern "C" fn pikachu_specialhi_exec(fighter: &mut L2CFighterCommon) -> L2CValue {
    // `stick_mul` IS AN ANGLE IN DEGREES, AND THIS WAS SETTING THE ROTATION TO ZERO.
    //
    // Transcribed from Game & Watch's own decompiled status script,
    // vendor/ssbu-status/.../lua2cpp_gamewatch.nro/gamewatch/gamewatch/status/SpecialHi/exec.c.
    // The sequence there is exactly:
    //
    //     angle = sign(stick_x) * param_special_hi/stick_mul        (only if |stick_x| > stick_min)
    //     PostureModule::set_rot(Vector3f(angle, 0, 0), 0)
    //     PostureModule::set_stick_lr(0.0)
    //     PostureModule::update_rot_y_lr()
    //     sv_kinetic_energy::set_angle(FIGHTER_KINETIC_ENERGY_ID_MOTION, math_rad(angle))
    //
    // This module had the guard and the three Posture calls right -- and passed a zero vector to
    // `set_rot` and skipped `set_angle` entirely, which is the whole of what the parameter does.
    // `stick_mul` was listed as NOT MODELLED on the grounds that a 30-unit multiplier could not
    // be interpreted; it is 30 DEGREES of launch-angle deviation, which is why it looked
    // implausible as a speed.
    //
    // So the up-B can now be aimed, which is the behaviour it has been missing since it was
    // ported, and the community reference port this module came from does not have either.
    let boma = fighter.module_accessor;
    let is_decide_direction =
        WorkModule::is_flag(boma, *FIGHTER_GAMEWATCH_STATUS_SPECIAL_HI_FLAG_DECIDE_DIRECTION);
    if is_decide_direction {
        // gamewatch param_special_hi stick_min = 0.25, stick_mul = 30
        let stick_min = 0.25;
        let stick_mul = 30.0f32;
        let raw_x = ControlModule::get_stick_x(boma);
        let stick_x = raw_x.abs();

        if stick_min <= stick_x {
            // SIGN IS INVERTED FROM THE OBVIOUS READING, and two independent things say so.
            //
            // The decompiled `exec.c` compares the stick against 0.0 and NEGATES on the branch
            // where the comparison succeeds -- so pushing right yields -30 and pushing left
            // yields +30. Playtest saw the same thing from the outside: "the angle goes in the
            // opposite direction as the direction the player pushed on the stick", against a
            // build that used `if raw_x >= 0.0 { +30 }`.
            //
            // Ghidra lost the comparison operator itself, so the decomp alone would leave this
            // 50/50. The report settles which way round it goes.
            let angle = if raw_x >= 0.0 { -stick_mul } else { stick_mul };
            PostureModule::set_rot(boma, &Vector3f { x: angle, y: 0.0, z: 0.0 }, 0);
            PostureModule::set_stick_lr(boma, 0.0);
            PostureModule::update_rot_y_lr(boma);
            // The rotation aims the MODEL; this aims the motion energy that actually carries him.
            // Degrees in the posture call, radians here, exactly as the source does it.
            sv_kinetic_energy!(
                set_angle, fighter, *FIGHTER_KINETIC_ENERGY_ID_MOTION, angle.to_radians()
            );
            crate::telemetry::probe::event(format!(
                "[rat] UPB aimed: stick_x={raw_x:.2} -> {angle:.0} degrees; {}",
                crate::telemetry::probe::ctx(boma)
            ));
        }

        WorkModule::off_flag(boma, *FIGHTER_GAMEWATCH_STATUS_SPECIAL_HI_FLAG_DECIDE_DIRECTION);
    }
    0.into()
}

unsafe extern "C" fn pikachu_specialhi_end(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    if fighter.global_table[0xb] == *FIGHTER_STATUS_KIND_LANDING_FALL_SPECIAL {
        // gamewatch param_special_hi landing_frame = 40
        WorkModule::set_float(boma, 40.0, *FIGHTER_INSTANCE_WORK_ID_FLOAT_LANDING_FRAME);
    }

    // THE ROTATION MUST BE PUT BACK, AND THIS NEVER DID IT.
    //
    // Playtest: "using gnw up b at an angle leaves pikachu standing at a slanted angle after
    // landing". Exactly so -- `_exec` aims him by rotating the posture and nothing ever undid it,
    // so the lean survived the move and every state after it.
    //
    // Game & Watch's own `SpecialHi_end` clears it unconditionally, before any of its
    // early-return paths:
    //
    //     PostureModule::set_rot(boma, Vector3f(0, 0, 0), 0)
    //
    // Unconditional matters. The two early returns below are about landing-lag bookkeeping, and
    // putting the reset after them -- which is where it would naturally go -- would leave the
    // lean in place on every path that takes one. That is most of them.
    PostureModule::set_rot(boma, &Vector3f { x: 0.0, y: 0.0, z: 0.0 }, 0);

    if fighter.global_table[0xb] != *FIGHTER_STATUS_KIND_LANDING {
        if fighter.global_table[0xb] != *FIGHTER_STATUS_KIND_LANDING_FALL_SPECIAL {
            return 0.into();
        }
    }

    0.into()
}

/// Watches for Pikachu being invisible during the up-B, restores him, and says so.
///
/// THE ROOT CAUSE IS FIXED ELSEWHERE, in tools/motion-patch/src/bin/synth.rs: the synthesised
/// `special_hi_open` entry named `expression_specialairhi1`, one of Pikachu's Quick Attack
/// scripts, and that script calls `VisibilityModule::set_whole(false)`. Synth now repoints
/// `scripts` alongside `game_script`, so nothing should hide him any more.
///
/// This stays anyway, for two reasons. The failure mode is a mod that looks completely broken --
/// three playtests in a row reported it -- and the cost is one call per frame during one move.
/// And it LOGS, so if anything hides him again the next log names the status and frame instead of
/// starting another round of "still invisible".
unsafe extern "C" fn pikachu_specialhi_visible(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let s = StatusModule::status_kind(boma);
    if s != *FIGHTER_STATUS_KIND_SPECIAL_HI
        && s != RAT_STATUS_KIND_SPECIAL_HI_OPEN
        && s != RAT_STATUS_KIND_SPECIAL_HI_CLOSE
    {
        return;
    }
    // IS HE ACTUALLY HIDDEN, OR IS THIS READING THE API WRONG?
    //
    // Round 3's log has this firing on the first frame of SPECIAL_HI_OPEN and _CLOSE every time,
    // and the user did NOT report an invisible up-B that round -- which is two different stories.
    // Either the synth fix worked and something else still hides him for a frame, or
    // `get_whole` does not mean what the `== 0` test assumes. It returns u64, not bool, and a
    // default-state return of 0 would make this fire harmlessly forever while looking like a bug.
    //
    // So it records the value BEFORE and AFTER the restore. If after reads 1 and before reads 0,
    // he was genuinely hidden and something upstream is still doing it. If both read the same,
    // the test is wrong and this guard is noise that should go.
    //
    // The one thing that is NOT the cause: Game & Watch's own up-B expression script hides a mesh
    // GROUP (`head` -> `head_close`, for the parachute pose) with `set_int64`. It never touches
    // whole-model visibility, and after the synth fix the entry does not name it anyway.
    let before = VisibilityModule::get_whole(boma);
    if before == 0 {
        VisibilityModule::set_whole(boma, true);
        crate::telemetry::probe::event(format!(
            "[rat] UPB visibility get_whole was {before}, set true -> now {}; {}",
            VisibilityModule::get_whole(boma),
            crate::telemetry::probe::ctx(boma)
        ));
    }
}

pub fn install() {
    Agent::new("pikachu")
        .on_line(Main, pikachu_specialhi_visible)
        .status(Pre, *FIGHTER_STATUS_KIND_SPECIAL_HI, pikachu_specialhi_pre)
        .status(Init, *FIGHTER_STATUS_KIND_SPECIAL_HI, pikachu_specialhi_init)
        .status(Main, *FIGHTER_STATUS_KIND_SPECIAL_HI, pikachu_specialhi_main)
        .status(Exec, *FIGHTER_STATUS_KIND_SPECIAL_HI, pikachu_specialhi_exec)
        .status(End, *FIGHTER_STATUS_KIND_SPECIAL_HI, pikachu_specialhi_end)
        .install();
}
