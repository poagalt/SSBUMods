// Ptooie's SHOT game script, transcribed from the binary, because the dump of it is not code.
//
// ALL FOUR SHOT MOTIONS RUN ONE BODY. In lua2cpp_packun, game_specialnshootf (0x138fd34c95),
// game_specialnshootb (0x1388be888c), game_specialairnshootf (0x16c46396bf) and
// game_specialairnshootb (0x16c30e52a6) are four thunks that all call FUN_71000d5940, so this is
// one function registered under the four names, the way acmd_speciallw registers its stance twice.
//
// WHY NOT GENERATED. vendor/SSBU-Dumped-Scripts renders this function's branches as
// `methodlib::L2CValue::operator==(lib::L2CValueconst&)const(false, true)` and its comparisons as
// bare `0x1646b0(0, 30)` calls, and it nests the distance bands inside the wrong arm of the first
// test -- the five arms below come out as one run of straight-line code with the shots repeated.
// So tools/gen-moves.sh takes only the effect and sound scripts for these motions (`fxsnd:` rows)
// and the game script is here, read off the decompile.
//
// WHAT IT DOES. A SEARCH box over his head for the whole shot. Then, from frame 1, one of six
// timings:
//
//   - shot straight out of the start, not from the wait (SHOOT_PREVIOUS_WAIT off): the ball
//     leaves on frame 11;
//   - shot from the wait, the ball is somewhere above him -- SHOOT_START_SPIKEBALL_DISTANCE, set
//     by the shoot status -- and the further away it is, the faster the lead-in, in bands of 30,
//     40, 50 and 100 times his scale. The two nearest bands release on frame 8, the rest on 11.
//
// THE SEARCH BOX IS SCALED HERE, BY HAND. tools/reach topscale scales every generated `top`
// anchor by the source fighter's scale over Pikachu's (0.87 / 0.96, tools/scales.tsv) and does so
// for the wait's identical box, but it only rewrites generated mod.rs files and is not idempotent,
// so it must never touch this one. `SEARCH_SCALE` is that ratio, applied to the same five numbers.
//
// Every arm clears the search on frame 10, raises SHOOT_BLOW_SPIKEBALL on 12 (the window
// src/specials/ptooie's shoot status reads to blow the ball out) and lowers it on 19, and frame 31
// restores the motion rate. The bands use PostureModule::scale, which is Pikachu's 0.96 here
// rather than Piranha Plant's 0.87 -- his own mechanism, run on the body it is attached to.
use {
    smash::{
        lua2cpp::*,
        phx::*,
        app::{sv_animcmd::*, lua_bind::*, *},
        lib::lua_const::*,
    },
    smash_script::*,
    smashline::{*, Priority::*}
};
use crate::acmd_ext as macros;

/// Piranha Plant's `scale` over Pikachu's, from tools/scales.tsv -- topscale's ratio for packun.
pub(super) const SEARCH_SCALE: f32 = 0.87 / 0.96;

/// His SEARCH box at `top`: radius 4, from (0, `y`, 0) to (0, `y2`, 0), at Pikachu's scale.
unsafe fn search(agent: &mut L2CAgentBase, y: f32, y2: f32) {
    let k = SEARCH_SCALE;
    macros::SEARCH(agent, 0, 0, Hash40::new("top"), 4.0 * k, 0.0, y * k, 0.0, 0.0, y2 * k, 0.0, *COLLISION_KIND_MASK_ATTACK, *HIT_STATUS_MASK_ALL, 1, *COLLISION_SITUATION_MASK_GA, *COLLISION_CATEGORY_MASK_ALL, *COLLISION_PART_MASK_ALL, true);
}

/// One arm of the shot: the motion rate it leads in at, the frame the ball leaves (8 or 11), and
/// the rates set on frames 11, 12 and 19.
struct Arm {
    lead: f32,
    release: f32,
    at_11: f32,
    at_12: f32,
    at_19: f32,
}

/// Not from the wait.
const STRAIGHT: Arm = Arm { lead: 1.0, release: 11.0, at_11: 3.0, at_12: 1.0, at_19: 1.1 };

/// From the wait, nearest first: the ball within `n * scale` of him takes that arm.
const BANDS: [(f32, Arm); 4] = [
    (30.0, Arm { lead: 0.6, release: 8.0, at_11: 2.0, at_12: 0.5, at_19: 1.4 }),
    (40.0, Arm { lead: 0.7, release: 8.0, at_11: 3.0, at_12: 0.7, at_19: 1.3 }),
    (50.0, Arm { lead: 0.9, release: 11.0, at_11: 4.0, at_12: 0.8, at_19: 1.2 }),
    (100.0, Arm { lead: 1.2, release: 11.0, at_11: 5.0, at_12: 1.0, at_19: 1.1 }),
];

/// From the wait, the ball further than every band.
const FAR: Arm = Arm { lead: 1.0, release: 11.0, at_11: 5.0, at_12: 1.0, at_19: 1.0 };

/// `ArticleModule::shoot_exist(SPIKEBALL, ALL, false)`, on Rat of Doom's clone of the ball.
unsafe fn shoot(agent: &mut L2CAgentBase) {
    if macros::is_excute(agent) {
        crate::articles::shoot_exist(agent.module_accessor, "spikeball", *ARTICLE_OPE_TARGET_ALL, false);
    }
}

unsafe fn run(agent: &mut L2CAgentBase, arm: &Arm) {
    macros::FT_MOTION_RATE(agent, arm.lead);
    if arm.release == 8.0 {
        frame(agent.lua_state_agent, 8.0);
        shoot(agent);
    }
    frame(agent.lua_state_agent, 10.0);
    if macros::is_excute(agent) {
        search!(agent, *MA_MSC_CMD_SEARCH_SEARCH_SCH_CLR, 0);
    }
    frame(agent.lua_state_agent, 11.0);
    macros::FT_MOTION_RATE(agent, arm.at_11);
    if arm.release == 11.0 {
        shoot(agent);
    }
    frame(agent.lua_state_agent, 12.0);
    macros::FT_MOTION_RATE(agent, arm.at_12);
    if macros::is_excute(agent) {
        WorkModule::on_flag(agent.module_accessor, *FIGHTER_PACKUN_STATUS_SPECIAL_N_FLAG_SHOOT_BLOW_SPIKEBALL);
    }
    frame(agent.lua_state_agent, 19.0);
    macros::FT_MOTION_RATE(agent, arm.at_19);
    if macros::is_excute(agent) {
        WorkModule::off_flag(agent.module_accessor, *FIGHTER_PACKUN_STATUS_SPECIAL_N_FLAG_SHOOT_BLOW_SPIKEBALL);
    }
}

unsafe extern "C" fn pikachu_game_specialnshoot(agent: &mut L2CAgentBase) {
    let boma = agent.module_accessor;
    if macros::is_excute(agent) {
        search(agent, 7.0, 8.0);
    }
    // His Kirby branch, kept so the transcription stays whole. A Pikachu never takes it.
    if WorkModule::get_int64(boma, *FIGHTER_INSTANCE_WORK_ID_INT_KIND) == *FIGHTER_KIND_KIRBY as u64 {
        if macros::is_excute(agent) {
            search(agent, 3.0, 3.0);
        }
    }
    frame(agent.lua_state_agent, 1.0);
    if !WorkModule::is_flag(boma, *FIGHTER_PACKUN_STATUS_SPECIAL_N_FLAG_SHOOT_PREVIOUS_WAIT) {
        run(agent, &STRAIGHT);
    } else {
        let distance = WorkModule::get_float(boma, *FIGHTER_PACKUN_STATUS_SPECIAL_N_WORK_FLOAT_SHOOT_START_SPIKEBALL_DISTANCE);
        let scale = PostureModule::scale(boma);
        let arm = BANDS
            .iter()
            .find(|(n, _)| distance <= n * scale)
            .map(|(_, arm)| arm)
            .unwrap_or(&FAR);
        run(agent, arm);
    }
    frame(agent.lua_state_agent, 31.0);
    macros::FT_MOTION_RATE(agent, 1.0);
}

pub fn install() {
    crate::charaslot::agent("pikachu")
        .game_acmd("game_specialnshootf", pikachu_game_specialnshoot, Default)
        .game_acmd("game_specialnshootb", pikachu_game_specialnshoot, Default)
        .game_acmd("game_specialairnshootf", pikachu_game_specialnshoot, Default)
        .game_acmd("game_specialairnshootb", pikachu_game_specialnshoot, Default)
        .install();
}
