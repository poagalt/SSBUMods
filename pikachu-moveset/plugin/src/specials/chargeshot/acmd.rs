// Charge Shot's ten `game_` scripts, HAND-WRITTEN -- and this is the one place in the moveset
// where that is the right answer rather than a shortcut.
//
// WHY NOT tools/gen-moves.sh. Every other ported move's ACMD is copied verbatim out of
// `vendor/SSBU-Dumped-Scripts`, which is the property that makes frame-data parity hold by
// construction. These five bodies cannot go through that pipeline, because they consist ENTIRELY
// of lines `tools/reach/src/bin/deforeign.rs` strips:
//
//     game_specialnstart    generate_article_enable(FIGHTER_SAMUS_GENERATE_ARTICLE_CSHOT, ...)
//                           WorkModule::on_flag(FIGHTER_SAMUS_..._FLAG_BULLET_DISP)
//     game_specialnhold     WorkModule::on_flag(FIGHTER_SAMUS_..._FLAG_CHARGE)
//     game_specialnfire     shoot_exist(FIGHTER_SAMUS_GENERATE_ARTICLE_CSHOT, OPE_TARGET_ALL, false)
//                           WorkModule::on_flag(FIGHTER_SAMUS_..._FLAG_SHOOT)
//     game_specialnfiremax  the same two lines as fire
//     game_specialncancel   WorkModule::off_flag(FIGHTER_SAMUS_..._FLAG_BULLET_DISP)
//
// and deforeign is RIGHT to strip them. `FIGHTER_SAMUS_GENERATE_ARTICLE_CSHOT` is 0, and article 0
// on Pikachu is his electro ball -- a generated script would have made his neutral-B spawn and
// fire a Thunder Jolt while the status code waited for a charge shot. Generation would therefore
// have produced five empty functions and a move that charges and never fires.
//
// So the article calls go through `super::generate` / `super::shoot`, which resolve the id the
// clone actually got, and the flags are written here directly -- they are STATUS work ids, which
// ADR 0006 allows because this moveset owns every line of all six statuses.
//
// THE FRAME NUMBERS ARE HERS AND ARE THE WHOLE OF THE TIMING: the article appears on frame 14 of
// the windup and leaves on frame 3 of the shot. Nothing else in these scripts is timing at all.
//
// THE AIR SCRIPTS ARE THE GROUND SCRIPTS. All five of her `SpecialAirN*` dumps are
// character-for-character the ground ones, so each pair shares a body and only the registered
// name differs.
//
// NO EFFECT OR SOUND SCRIPTS. Hers are `samus_*` -- `samus_cshot_max`, `se_samus_*` -- and ADR
// 0002 keeps Pikachu's own effect archives, so there is nothing to register. The consequence is
// stated in the module header: the projectile and the full-charge glow are invisible.
use {
    smash::{
        app::{lua_bind::*, sv_animcmd::*},
        lib::lua_const::*,
        lua2cpp::L2CAgentBase,
    },
    smash_script::*,
    smashline::{Priority::*, *},
};

/// `game_specialnstart` / `game_specialairnstart`, samus.
unsafe extern "C" fn game_specialnstart(agent: &mut L2CAgentBase) {
    frame(agent.lua_state_agent, 14.0);
    if macros::is_excute(agent) {
        super::generate(agent.module_accessor);
    }
}

/// `game_specialnhold` / `game_specialairnhold`, samus. One flag, and the charge loop's status
/// code does not read it -- the count is in `state`. Kept because the flag is part of the move's
/// own record of itself and costs one line.
unsafe extern "C" fn game_specialnhold(agent: &mut L2CAgentBase) {
    if macros::is_excute(agent) {
        WorkModule::on_flag(
            agent.module_accessor,
            *FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_CHARGE,
        );
    }
}

/// `game_specialnfire` / `game_specialnfiremax` and their air twins, samus. All four are the same
/// two lines on frame 3.
unsafe extern "C" fn game_specialnfire(agent: &mut L2CAgentBase) {
    frame(agent.lua_state_agent, 3.0);
    if macros::is_excute(agent) {
        super::shoot(agent.module_accessor);
    }
}

/// `game_specialncancel` / `game_specialairncancel`, samus.
unsafe extern "C" fn game_specialncancel(agent: &mut L2CAgentBase) {
    if macros::is_excute(agent) {
        WorkModule::off_flag(
            agent.module_accessor,
            *FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_BULLET_DISP,
        );
    }
}

pub fn install() {
    crate::charaslot::agent("pikachu")
        .game_acmd("game_specialnstart", game_specialnstart, Default)
        .game_acmd("game_specialairnstart", game_specialnstart, Default)
        .game_acmd("game_specialnhold", game_specialnhold, Default)
        .game_acmd("game_specialairnhold", game_specialnhold, Default)
        .game_acmd("game_specialnfire", game_specialnfire, Default)
        .game_acmd("game_specialairnfire", game_specialnfire, Default)
        .game_acmd("game_specialnfiremax", game_specialnfire, Default)
        .game_acmd("game_specialairnfiremax", game_specialnfire, Default)
        .game_acmd("game_specialncancel", game_specialncancel, Default)
        .game_acmd("game_specialairncancel", game_specialncancel, Default)
        .install();
}
