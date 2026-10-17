// Revenge's stance script, hand-written rather than generated.
//
// It mirrors Incineroar's game_speciallwstart exactly -- same FT_MOTION_RATE curve, same frame
// numbers -- but replaces the two work flags with calls into ../speciallw, which owns the counter
// window.
//
// WHY HAND-WRITTEN. The generated port could not carry the window at all. Incineroar's script
// raises FIGHTER_GAOGAEN_STATUS_SPECIAL_LW_FLAG_STANCE_START on frame 8 and _STANCE_END on frame
// 28, and his C++ reads those. deforeign strips them, correctly: those ids are per-fighter and
// mean something else on Pikachu. That left the window with no source of truth, and deriving it
// from MotionModule::frame in the status loop was wrong because FT_MOTION_RATE(0.25) runs from
// frame 1 to 9 -- at script frame 8 the motion frame is about 2.75, so an 8..28 motion-frame
// window actually opened around script frame 13.
//
// ACMD frame() calls ARE script frames, so the window now opens and closes on exactly the frames
// Incineroar uses. The rest of the script is his, unchanged.
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

unsafe extern "C" fn pikachu_game_speciallwstart(agent: &mut L2CAgentBase) {
    frame(agent.lua_state_agent, 1.0);
    macros::FT_MOTION_RATE(agent, 0.25);
    frame(agent.lua_state_agent, 8.0);
    if macros::is_excute(agent) {
        // Incineroar: WorkModule::on_flag(FLAG_STANCE_START)
        super::speciallw::open_window(agent.module_accessor);
    }
    frame(agent.lua_state_agent, 9.0);
    macros::FT_MOTION_RATE(agent, 1.5);
    frame(agent.lua_state_agent, 21.0);
    macros::FT_MOTION_RATE(agent, 1.0);
    frame(agent.lua_state_agent, 28.0);
    if macros::is_excute(agent) {
        // Incineroar: WorkModule::on_flag(FLAG_STANCE_END)
        super::speciallw::close_window(agent.module_accessor);
    }
    macros::FT_MOTION_RATE(agent, 0.5);
    frame(agent.lua_state_agent, 46.0);
    macros::FT_MOTION_RATE(agent, 1.0);
}

pub fn install() {
    crate::charaslot::agent("pikachu")
        .game_acmd("game_speciallwstart", pikachu_game_speciallwstart, Default)
        // The AIR stance runs the identical script -- Incineroar's SpecialAirLwStart has the same
        // FT_MOTION_RATE curve and raises STANCE_START on 8 and STANCE_END on 28 exactly as the
        // ground one does -- so it is registered against the same function rather than copied.
        .game_acmd("game_specialairlwstart", pikachu_game_speciallwstart, Default)
        .install();
}
