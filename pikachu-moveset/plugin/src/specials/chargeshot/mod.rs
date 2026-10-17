// Samus's Charge Shot, transcribed from her own status tree (ADR 0006).
//
// WHY SAMUS AND NOT PIRANHA PLANT. The brief was "research both Ptooie and Samus charge shot, and
// implement whichever one will be easier". Both are a charge-and-release neutral-B built on a
// projectile article, so the deciding question was which article can be cloned onto Pikachu at
// all -- and `tools/check-article-donor-params.sh` answers it without a boot:
//
//     cshot      WEAPON_KIND_SAMUS_CSHOT      donor samus   entry 0x118   clean
//     spikeball  WEAPON_KIND_PACKUN_SPIKEBALL donor packun  entry 0x1228  UNGUARDED in 2 fn(s)
//
// "Unguarded" means `WeaponSpecializer_PackunSpikeball` reads Piranha Plant's
// `FighterParamAccessor2` entry with the fighter kind compiled in, which is the exact shape of the
// crash `docs/the-article-path.md` Part 12 root-caused. Samus's is clean in all 114 audited slots.
// That is not a preference; it is the difference between a move and a crash.
//
// THE SHAPE OF THE MOVE, in her own six statuses:
//
//     SPECIAL_N            the windup. Its ACMD generates the CSHOT article on frame 14. At the
//                          end of the animation: already at full charge -> _F, else -> _H.
//     SPECIAL_N_H          the charge loop. Holds the pose, counts the charge up one per frame,
//                          and watches for every way out: release the button -> _F, shield /
//                          dodge / roll / jump -> _C or _JUMP_CANCEL, charge hits the cap -> _E.
//     SPECIAL_N_E          the cap. Same functions as the cancel; she stops charging and stands.
//     SPECIAL_N_C          the cancel. Plays `special_n_c` and remembers WHICH cancel in status
//                          work so its own loop can hand off to shield / dodge / jump.
//     SPECIAL_N_JUMP_CANCEL  the air jump out of a charge. Its own status because it ends by
//                          choosing between JUMP_AERIAL, FLY and FALL.
//     SPECIAL_N_F          the shot. Its ACMD fires the article on frame 3; `exec` applies the
//                          recoil on the frame it leaves, scaled by how long she charged.
//
// WHAT THIS PORT DOES NOT REPRODUCE, stated here rather than discovered in a playtest:
//
//   * THE CHARGE GLOW. `SpecialNH_exit` requests `charge_max` and follows `samus_cshot_max` on her
//     `armr` bone once the charge is full. Those are SAMUS's effects and ADR 0002 keeps Pikachu's
//     own effect archives, so the visual cue that the shot is fully charged is absent. The
//     `EFH_CHARGE_MAX` handle it would need is still kept in `state`, so turning it on later is a
//     line rather than a re-derivation.
//   * THE PROJECTILE'S OWN VISUALS, for the same reason, and more completely: the cshot article
//     has NO MODEL IN THE ARC AT ALL. `fighter/samus/model/cshot` is not a path in the archive --
//     only `motion/cshot/cNN/motion_list.bin` is -- so the ball Samus fires is made entirely of
//     her effects. The cloned article keeps its hitbox and its motion and will be invisible.
//   * KIRBY'S COPY ABILITY. `SpecialNH_exit` branches on the owner being SAMUSD, or KIRBY with
//     `COPY_CHARA == SAMUSD`, purely to pick `samusd_cshot_max` over `samus_cshot_max`. With
//     neither effect requested there is nothing for the branch to choose between.
//
// THE WORK IDS SPLIT THE WAY ADR 0006 SAYS. Her three INSTANCE ids -- ST_INIT, EFH_CHARGE_MAX and
// the stored charge SPECIAL_N_COUNT -- all land inside PIKACHU's own instance block, so they go
// through `state` and never through `WorkModule`. Her STATUS ids (`..._STATUS_SPECIAL_N_*`) are
// used directly, because this moveset registers every line of all six statuses and no Pikachu code
// runs inside them. See ../whip/state.rs for the collision that established the rule.
pub mod acmd;
pub mod state;

use super::status::*;
use {
    smash::{
        app::{lua_bind::*, sv_kinetic_energy, ArticleOperationTarget, BattleObjectModuleAccessor,
              GroundCliffCheckKind, GroundCorrectKind, SituationKind},
        lib::{lua_const::*, L2CValue},
        lua2cpp::*,
        phx::Hash40,
    },
    smashline::{Priority::*, *},
};

/// `globalTable[0x16]` is the CURRENT situation and `[0x17]` the PREVIOUS one. Named because every
/// one of these statuses reads both and a bare `0x16` is unreadable; the pair is what makes the
/// ground/air swaps below TRANSITIONS rather than states.
const GT_SITUATION: i32 = 0x16;
const GT_SITUATION_PREV: i32 = 0x17;
/// `globalTable[0x20]` and `[0x21]` are the cat1 and cat2 command flags for the frame -- the same
/// two the cargo carry reads in ../shoulder, and the engine's own answer to "what did the player
/// ask for", accounting for buffering and per-controller deadzones.
const GT_CAT1: i32 = 0x20;
const GT_CAT2: i32 = 0x21;

unsafe fn entry_of(boma: *mut BattleObjectModuleAccessor) -> usize {
    let e = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if e >= 8 { 0 } else { e }
}

unsafe fn on_ground(fighter: &mut L2CFighterCommon) -> bool {
    fighter.global_table[GT_SITUATION].get_i32() == *SITUATION_KIND_GROUND
}

/// `hash40("param_special_n")`, the param object every number in this move comes out of.
const PARAM_SPECIAL_N: u64 = 0x0f_8991_92aa;
/// `hash40("cshot_charge_frame")` -- samus's is 112, and it is both the charge cap and, read as a
/// float, the divisor that turns the stored charge into a 0..1 ratio.
const CSHOT_CHARGE_FRAME: u64 = 0x12_04ac_f3ae;
/// `hash40("cshot_shot_spd_min")` / `_max` -- the recoil, -0.1 at no charge and -0.8 at full.
const CSHOT_SHOT_SPD_MIN: u64 = 0x12_1387_c0df;
const CSHOT_SHOT_SPD_MAX: u64 = 0x12_2f8a_ff86;
/// The fifth key in her `param_special_n` has NO LABEL in `vendor/param-labels/ParamLabels.csv`,
/// so it is named here by what it is used for rather than by a name nobody has. `SpecialNC_end`
/// reads it and writes it into `FIGHTER_INSTANCE_WORK_ID_INT_AIR_ESCAPE_ADD_XLU_START_FRAME`; in
/// her prc it is 0.
const CSHOT_AIR_ESCAPE_ADD_XLU_START_FRAME: u64 = 0x2b_f59d_b69d;
/// `hash40("aerial_type")`, a FIGHTER-level param -- read as `get_param_int(<the name>, 0)`, which
/// is the shape that distinguishes a fighter param from a `param_special_n` field. It decides
/// whether a double jump is an ordinary `JUMP_AERIAL` or a hover (`FLY`).
const AERIAL_TYPE: u64 = 0x0b_99cc_3fbc;

/// The charge cap, in frames. Read through the param provider rather than written here, because
/// `src/params` already answers `param_special_n` for a Pikachu and a second copy of 112 in this
/// file is a second thing to keep in step.
unsafe fn charge_frame(boma: *mut BattleObjectModuleAccessor) -> f32 {
    WorkModule::get_param_float(boma, PARAM_SPECIAL_N, CSHOT_CHARGE_FRAME)
}

/// The cloned article's id on Pikachu.
///
/// NOT `FIGHTER_SAMUS_GENERATE_ARTICLE_CSHOT`, WHICH IS 0 AND IS PIKACHU'S ELECTRO BALL. That is
/// exactly the collision `tools/reach/src/bin/deforeign.rs` exists to catch, and it does: the
/// three `ArticleModule` calls in her ACMD name that constant and are STRIPPED out of the
/// generated scripts. So the generate, the shot and the teardown are all done here instead, from
/// the id `src/articles` actually assigned the clone.
///
/// `None` when the row is disabled, and every caller treats that as "no article" rather than
/// falling back to an index -- an article call with a wrong id is the failure that looks like a
/// working move firing somebody else's projectile.
fn cshot() -> Option<i32> {
    crate::articles::article_id("cshot")
}

unsafe fn remove_cshot(boma: *mut BattleObjectModuleAccessor) {
    if let Some(id) = cshot() {
        ArticleModule::remove_exist(boma, id, ArticleOperationTarget(0));
    }
}

// ================================================================================================
// SPECIAL_N -- the windup
// ================================================================================================

/// `SpecialN_pre`, samus.
///
///     sub_status_pre_SpecialNCommon()
///     WorkModule::on_flag(FIGHTER_SAMUS_INSTANCE_WORK_ID_FLAG_ST_INIT)
///     StatusModule::init_settings(SITUATION_KIND_NONE, FIGHTER_KINETIC_TYPE_UNIQ,
///         GROUND_CORRECT_KIND_KEEP, GROUND_CLIFF_CHECK_KIND_NONE, true,
///         *_KEEP_FLAG_ALL_FLAG, *_KEEP_FLAG_ALL_INT, *_KEEP_FLAG_ALL_FLOAT, 0)
///     FighterStatusModuleImpl::set_fighter_status_data(false, FIGHTER_TREADED_KIND_NO_REAC,
///         false, false, false, ACTION_TRIGGER_ON | ATTACK_KIND_SPECIAL_N,
///         FIGHTER_STATUS_ATTR_START_TURN, FIGHTER_POWER_UP_ATTACK_BIT_SPECIAL_N, 0)
///
/// `KEEP_FLAG_ALL_*` is the line that makes the whole move possible: the five statuses hand each
/// other the charge count and the cancel type in STATUS work, and without it the engine would
/// clear that region on every transition between them.
unsafe extern "C" fn special_n_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    fighter.sub_status_pre_SpecialNCommon();
    state::set_st_init(entry_of(boma), true);
    init_settings_common(fighter, *FIGHTER_STATUS_ATTR_START_TURN as u32);
    0.into()
}

/// The `init_settings` + `set_fighter_status_data` pair every one of the six statuses opens with.
///
/// Folded because her six `_pre` functions are byte-identical apart from `SpecialN_pre`'s extra
/// `sub_status_pre_SpecialNCommon` call, its `ST_INIT` raise and its `FIGHTER_STATUS_ATTR_START_TURN`
/// -- everything else, including the three `KEEP_FLAG_ALL_*`, is the same in all six. Six copies
/// of eighteen arguments is six chances for one of them to drift from the other five.
unsafe fn init_settings_common(fighter: &mut L2CFighterCommon, attr: u32) {
    let boma = fighter.module_accessor;
    StatusModule::init_settings(
        boma,
        SituationKind(*SITUATION_KIND_NONE),
        *FIGHTER_KINETIC_TYPE_UNIQ,
        *GROUND_CORRECT_KIND_KEEP as u32,
        GroundCliffCheckKind(*GROUND_CLIFF_CHECK_KIND_NONE),
        true,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_ALL_FLAG,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_ALL_INT,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_ALL_FLOAT,
        0,
    );
    FighterStatusModuleImpl::set_fighter_status_data(
        boma,
        false,
        *FIGHTER_TREADED_KIND_NO_REAC,
        false,
        false,
        false,
        (*FIGHTER_LOG_MASK_FLAG_ACTION_TRIGGER_ON | *FIGHTER_LOG_MASK_FLAG_ATTACK_KIND_SPECIAL_N)
            as u64,
        attr,
        *FIGHTER_POWER_UP_ATTACK_BIT_SPECIAL_N as u32,
        0,
    );
}

/// `SpecialN_init`, samus. Guarded so it runs once per press even though the engine may reach it
/// again: `ST_INIT` is raised by `pre` and cleared at the bottom of this.
///
///     if WorkModule::is_flag(ST_INIT) then
///         WorkModule::clear_all(WORK_KIND_STATUS)
///         WorkModule::set_int(0, FIGHTER_SAMUS_STATUS_SPECIAL_N_WORK_INT_COUNT)
///         WorkModule::on_flag(FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_MOT_RESTART)
///         if globalTable[0x16] == SITUATION_KIND_GROUND then off_flag(FLAG_AIR_START)
///                                                       else on_flag(FLAG_AIR_START)
///         WorkModule::off_flag(ST_INIT)
///     end
unsafe extern "C" fn special_n_init(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    if !state::st_init(entry) {
        return 0.into();
    }
    WorkModule::clear_all(boma, smash::app::WorkKind(*WORK_KIND_STATUS_WORK));
    WorkModule::set_int(boma, 0, *FIGHTER_SAMUS_STATUS_SPECIAL_N_WORK_INT_COUNT);
    WorkModule::on_flag(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_MOT_RESTART);
    if on_ground(fighter) {
        WorkModule::off_flag(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_AIR_START);
    } else {
        WorkModule::on_flag(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_AIR_START);
    }
    state::set_st_init(entry, false);
    0.into()
}

/// The ground/air setup every one of these statuses opens with, and which four of their loops
/// repeat on a situation CHANGE.
///
/// Folded into one helper because it is character-for-character the same in `SpecialN_main`,
/// `SpecialNH_main`, `SpecialNC_main`, `SpecialNE_main` and `SpecialNF_main` apart from the two
/// motion names -- and because writing it five times is five chances for one of them to drift.
/// `restart` carries `FLAG_MOT_RESTART`: set, the motion starts from frame 0; clear, the new
/// motion inherits the frame the old one was on, which is what keeps the pose continuous when she
/// takes off or lands mid-charge.
unsafe fn ground_air_setup(
    boma: *mut BattleObjectModuleAccessor,
    ground: bool,
    ground_motion: &str,
    air_motion: &str,
    ground_correct: i32,
    restart: bool,
) {
    let motion = if ground { ground_motion } else { air_motion };
    if ground {
        GroundModule::correct(boma, GroundCorrectKind(ground_correct));
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_GROUND_STOP);
    } else {
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_AIR_STOP);
    }
    if restart {
        MotionModule::change_motion(boma, Hash40::new(motion), 0.0, 1.0, false, 0.0, false, false);
        WorkModule::off_flag(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_MOT_RESTART);
    } else {
        MotionModule::change_motion_inherit_frame(
            boma, Hash40::new(motion), -1.0, 1.0, 0.0, false, false,
        );
    }
}

/// `SpecialN_main`, samus. The ground branch uses `GROUND_CORRECT_KIND_GROUND_CLIFF_STOP_ATTACK`
/// -- she will not walk off a ledge while winding up -- where the charge hold below uses plain
/// `GROUND_CLIFF_STOP`. That is a real difference between the two statuses, not a slip.
unsafe extern "C" fn special_n_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let restart = WorkModule::is_flag(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_MOT_RESTART);
    ground_air_setup(
        boma,
        on_ground(fighter),
        "special_n_s",
        "special_air_n_s",
        *GROUND_CORRECT_KIND_GROUND_CLIFF_STOP_ATTACK,
        restart,
    );
    ControlModule::set_add_jump_mini_button_life(boma, 8);
    fighter.sub_shift_status_main(L2CValue::Ptr(special_n_main_loop as *const () as _))
}

/// The situation-change swap the four charge loops share, samus `0x71000223c8` and its siblings.
///
///     if globalTable[0x17] == GROUND and globalTable[0x16] == AIR    -> go airborne
///     if globalTable[0x17] != GROUND and globalTable[0x16] == GROUND -> go grounded
///
/// A TRANSITION, NOT A STATE, and getting that wrong is a mistake this project has already made
/// once in the cargo throw -- see the long note above `throw_loop` in ../shoulder. It fires on the
/// frame the situation flips and on no other.
unsafe fn situation_swap(
    fighter: &mut L2CFighterCommon,
    ground_motion: &str,
    air_motion: &str,
    ground_correct: i32,
) {
    let boma = fighter.module_accessor;
    let now_ground = fighter.global_table[GT_SITUATION].get_i32() == *SITUATION_KIND_GROUND;
    let was_ground = fighter.global_table[GT_SITUATION_PREV].get_i32() == *SITUATION_KIND_GROUND;
    if now_ground == was_ground {
        return;
    }
    ground_air_setup(boma, now_ground, ground_motion, air_motion, ground_correct, false);
}

/// `SpecialN_main_loop`, samus: the situation swap, then "when the windup animation ends, decide".
unsafe extern "C" fn special_n_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    situation_swap(
        fighter,
        "special_n_s",
        "special_air_n_s",
        *GROUND_CORRECT_KIND_GROUND_CLIFF_STOP_ATTACK,
    );
    if !MotionModule::is_end(boma) {
        return 0.into();
    }
    special_n_windup_done(fighter)
}

/// samus `0x7100024530`, the function `SpecialN_main_loop` fastshifts to at the end of the windup.
///
/// NOT IN THE PUBLIC STATUS DUMP -- that only carries symbolised entry points -- so it was read
/// out of `work/arc-nro/prebuilt/nro/release/lua2cpp_samus.nro` with tools/disasm:
///
///     <the same situation swap as the loop>
///     count = WorkModule::get_int(FIGHTER_SAMUS_INSTANCE_WORK_ID_INT_SPECIAL_N_COUNT)
///     if FUN_7100005770(this) <= count then
///         WorkModule::on_flag(FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_MOT_RESTART)
///         change_status(FIGHTER_SAMUS_STATUS_KIND_SPECIAL_N_F, false)
///         return true
///     end
///     change_status(FIGHTER_SAMUS_STATUS_KIND_SPECIAL_N_H, ...)
///
/// and `FUN_7100005770` is one call:
/// `WorkModule::get_param_float(param_special_n, cshot_charge_frame)`.
///
/// SO A STORED FULL CHARGE FIRES IMMEDIATELY. That is the behaviour everyone knows from the move
/// and it is one comparison; missing it would have made a fully-charged Pikachu re-enter the
/// charge loop and need a second release, which reads as the charge having been lost.
///
/// Called directly rather than through `fastshift`, because the only thing `fastshift` adds here
/// is running it on the same frame -- which is what calling it does.
unsafe extern "C" fn special_n_windup_done(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    situation_swap(
        fighter,
        "special_n_s",
        "special_air_n_s",
        *GROUND_CORRECT_KIND_GROUND_CLIFF_STOP_ATTACK,
    );
    let count = state::count(entry_of(boma));
    if charge_frame(boma) <= count as f32 {
        WorkModule::on_flag(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_MOT_RESTART);
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_N_F.into(), false.into());
        return 1.into();
    }
    fighter.change_status(RAT_STATUS_KIND_SPECIAL_N_H.into(), false.into());
    1.into()
}

/// `SpecialN_end`, samus: `return 0`. Registered anyway, because a status this moveset does not
/// fully own is a status Pikachu's own code still runs lines in -- the rule the up-B paid for.
unsafe extern "C" fn special_n_end(_fighter: &mut L2CFighterCommon) -> L2CValue {
    0.into()
}

/// The teardown both `SpecialN_exit` and `SpecialNH_exit` reach when the move is being ABANDONED
/// rather than continued -- the player got hit, grabbed, or otherwise left for a status that is
/// not one of this move's.
///
///     WorkModule::set_int(0, FIGHTER_SAMUS_INSTANCE_WORK_ID_INT_SPECIAL_N_COUNT)
///     ArticleModule::remove_exist(FIGHTER_SAMUS_GENERATE_ARTICLE_CSHOT, 0)
///     EffectModule::remove_common(hash40("charge_max"))
///     <MA_MSC_EFFECT_REMOVE on EFH_CHARGE_MAX>
///
/// THE CHARGE IS LOST, WHICH IS CORRECT AND IS WORTH SAYING OUT LOUD: in the real move, being hit
/// out of a charge keeps nothing. The stored charge only survives a DELIBERATE exit -- firing, or
/// one of the cancels -- and the branch above each call site is what tells them apart.
///
/// The two effect removals are omitted: nothing in this port ever requests `charge_max`, so there
/// is nothing to remove, and `remove_common` with a name out of another fighter's effect archive
/// is a call this moveset does not make. See the header.
unsafe fn abandon_charge(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    state::set_count(entry_of(boma), 0);
    remove_cshot(boma);
}

/// `SpecialN_exit`, samus.
///
///     if next_status != SPECIAL_N_F and next_status != SPECIAL_N_H then <abandon> end
///
/// Leaving the windup for the shot or for the hold is the move continuing; anything else is the
/// move ending, and only then is the charge thrown away and the article removed.
unsafe extern "C" fn special_n_exit(fighter: &mut L2CFighterCommon) -> L2CValue {
    let next = StatusModule::status_kind_next(fighter.module_accessor);
    if next != RAT_STATUS_KIND_SPECIAL_N_F && next != RAT_STATUS_KIND_SPECIAL_N_H {
        abandon_charge(fighter);
    }
    0.into()
}

// ================================================================================================
// SPECIAL_N_H -- the charge
// ================================================================================================

/// `SpecialNH_pre`, samus -- `SpecialN_pre` without the `sub_status_pre_SpecialNCommon` call, the
/// `ST_INIT` raise or the START_TURN attribute, because the charge is a continuation of the windup
/// rather than a new press. The same `_pre` serves `_C`, `_E`, `_F` and `_JUMP_CANCEL`, which is
/// what her own tree does.
unsafe extern "C" fn special_n_h_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    init_settings_common(fighter, 0);
    0.into()
}

/// `SpecialNH_main`, samus.
///
/// THE TWELVE TRANSITION TERMS ARE THE MOVE'S ENTIRE CANCEL SET, and they are data rather than
/// logic: without them the engine refuses the transitions, and the loop below can test for a
/// shield or a jump all it likes and never be allowed to take one.
unsafe extern "C" fn special_n_h_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    ground_air_setup(
        boma,
        on_ground(fighter),
        "special_n_h",
        "special_air_n_h",
        *GROUND_CORRECT_KIND_GROUND_CLIFF_STOP,
        // Her `main` always starts the hold from frame 0 -- it calls `change_motion`, not
        // `change_motion_inherit_frame`, and does not consult `FLAG_MOT_RESTART` at all.
        true,
    );
    for term in [
        *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_GUARD_ON,
        *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_ESCAPE,
        *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_ESCAPE_F,
        *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_ESCAPE_B,
        *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_JUMP_SQUAT_BUTTON,
        *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_JUMP_SQUAT,
        *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_JUMP_AERIAL,
        *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_JUMP_AERIAL_BUTTON,
        *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_FLY_BUTTON,
        *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_FLY,
        *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_FLY_NEXT,
        *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_ESCAPE_AIR,
    ] {
        WorkModule::enable_transition_term(boma, term);
    }
    ControlModule::set_add_jump_mini_button_life(boma, 8);
    fighter.sub_shift_status_main(L2CValue::Ptr(special_n_h_main_loop as *const () as _))
}

/// Take one of the cancels: record WHICH one in status work, then change status.
///
/// Every one of the seven cancel branches in her loop has this shape, and every one also has an
/// `else` that writes `CANCEL_TYPE_NONE` when the transition term is NOT enabled -- and then
/// changes status anyway. That looks like a bug and is not: `SpecialNC_main_loop` reads the type
/// back and only hands off to shield / jump when it is a real one, so `NONE` means "play the
/// cancel animation and then just stand up".
unsafe fn take_cancel(
    fighter: &mut L2CFighterCommon,
    allowed: bool,
    kind: i32,
    status: i32,
) -> L2CValue {
    let boma = fighter.module_accessor;
    let ty = if allowed { kind } else { *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_NONE };
    WorkModule::set_int(boma, ty, *FIGHTER_SAMUS_STATUS_SPECIAL_N_WORK_INT_CANCEL_TYPE);
    fighter.change_status(status.into(), false.into());
    1.into()
}

/// `SpecialNH_main_loop`, samus -- 667 lines of decompiled C, and the shape is:
///
///     <situation swap, on special_n_h / special_air_n_h>
///     released = not (SPECIAL or ATTACK held)
///     if not released then
///         if on ground:
///             stick escape / escape_f / escape_b   -> SPECIAL_N_C with the matching CANCEL_TYPE
///             jump button, or flick jump           -> SPECIAL_N_C, CANCEL_TYPE_GROUND_JUMP
///             guard command                        -> SPECIAL_N_C, CANCEL_TYPE_GROUND_GUARD
///         else:
///             guard trigger and escape-air allowed -> SPECIAL_N_C, CANCEL_TYPE_AIR_ESCAPE_AIR
///             jump with jumps left                 -> SPECIAL_N_JUMP_CANCEL, AIR_JUMP_AERIAL
///         <otherwise: count up, and at the cap -> SPECIAL_N_E>
///     else
///         change_motion(special_n_f / special_air_n_f); change_status(SPECIAL_N_F)
///     end
///
/// THE RELEASE TEST IS TWO BUTTONS, and that is not a detail: Charge Shot fires on releasing the
/// special button OR the attack button, which is what lets a player who buffered an attack out of
/// the charge get the shot rather than a jab.
///
/// THE JUMP HAS TWO DOORS AND SHE TESTS BOTH -- the jump BUTTON, and, only when the controller has
/// flick-jump enabled, the jump COMMAND off the stick. Testing one would make the cancel work on
/// half the control schemes, which is the exact shape of bug the cargo throw's stick reading
/// shipped once already.
unsafe extern "C" fn special_n_h_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    situation_swap(
        fighter,
        "special_n_h",
        "special_air_n_h",
        *GROUND_CORRECT_KIND_GROUND_CLIFF_STOP,
    );

    let holding = ControlModule::check_button_on(boma, *CONTROL_PAD_BUTTON_SPECIAL)
        || ControlModule::check_button_on(boma, *CONTROL_PAD_BUTTON_ATTACK);

    if !holding {
        // RELEASED. She changes the motion HERE, before the status change, so the shot's first
        // frame is already the shot's pose -- `SpecialNF_main` then finds `FLAG_MOT_RESTART` clear
        // and inherits the frame rather than snapping back to 0.
        let motion = if on_ground(fighter) { "special_n_f" } else { "special_air_n_f" };
        MotionModule::change_motion(boma, Hash40::new(motion), 0.0, 1.0, false, 0.0, false, false);
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_N_F.into(), false.into());
        return 1.into();
    }

    let cat1 = fighter.global_table[GT_CAT1].get_i32();
    let cat2 = fighter.global_table[GT_CAT2].get_i32();

    if on_ground(fighter) {
        for (mask, term, kind) in [
            (
                *FIGHTER_PAD_CMD_CAT2_FLAG_STICK_ESCAPE,
                *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_ESCAPE,
                *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_GROUND_ESCAPE,
            ),
            (
                *FIGHTER_PAD_CMD_CAT2_FLAG_STICK_ESCAPE_F,
                *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_ESCAPE_F,
                *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_GROUND_ESCAPE_F,
            ),
            (
                *FIGHTER_PAD_CMD_CAT2_FLAG_STICK_ESCAPE_B,
                *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_ESCAPE_B,
                *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_GROUND_ESCAPE_B,
            ),
        ] {
            if cat2 & mask != 0 {
                let ok = WorkModule::is_enable_transition_term(boma, term);
                return take_cancel(fighter, ok, kind, RAT_STATUS_KIND_SPECIAL_N_C);
            }
        }
        if cat1 & *FIGHTER_PAD_CMD_CAT1_FLAG_JUMP_BUTTON != 0 {
            if fighter.sub_check_button_jump().get_bool() {
                return take_cancel(
                    fighter,
                    true,
                    *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_GROUND_JUMP,
                    RAT_STATUS_KIND_SPECIAL_N_C,
                );
            }
        } else if ControlModule::is_enable_flick_jump(boma)
            && cat1 & *FIGHTER_PAD_CMD_CAT1_FLAG_JUMP != 0
            && fighter.sub_check_button_frick().get_bool()
        {
            return take_cancel(
                fighter,
                true,
                *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_GROUND_JUMP,
                RAT_STATUS_KIND_SPECIAL_N_C,
            );
        }
        if fighter.sub_check_command_guard().get_bool() {
            let ok = WorkModule::is_enable_transition_term(
                boma,
                *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_GUARD_ON,
            );
            return take_cancel(
                fighter,
                ok,
                *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_GROUND_GUARD,
                RAT_STATUS_KIND_SPECIAL_N_C,
            );
        }
    } else {
        if ControlModule::check_button_trigger(boma, *CONTROL_PAD_BUTTON_GUARD)
            && !WorkModule::is_flag(boma, *FIGHTER_INSTANCE_WORK_ID_FLAG_DISABLE_ESCAPE_AIR)
        {
            let ok = WorkModule::is_enable_transition_term(
                boma,
                *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_ESCAPE_AIR,
            );
            return take_cancel(
                fighter,
                ok,
                *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_AIR_ESCAPE_AIR,
                RAT_STATUS_KIND_SPECIAL_N_C,
            );
        }
        let jumps = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_JUMP_COUNT);
        let jumps_max = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_JUMP_COUNT_MAX);
        if jumps < jumps_max {
            if cat1 & *FIGHTER_PAD_CMD_CAT1_FLAG_JUMP != 0
                && ControlModule::is_enable_flick_jump(boma)
            {
                let ok = WorkModule::is_enable_transition_term(
                    boma,
                    *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_JUMP_AERIAL,
                );
                return take_cancel(
                    fighter,
                    ok,
                    *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_AIR_JUMP_AERIAL,
                    RAT_STATUS_KIND_SPECIAL_N_JUMP_CANCEL,
                );
            }
            if cat1 & *FIGHTER_PAD_CMD_CAT1_FLAG_JUMP_BUTTON != 0 {
                let ok = WorkModule::is_enable_transition_term(
                    boma,
                    *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_JUMP_AERIAL_BUTTON,
                );
                return take_cancel(
                    fighter,
                    ok,
                    *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_AIR_JUMP_AERIAL,
                    RAT_STATUS_KIND_SPECIAL_N_JUMP_CANCEL,
                );
            }
        }
    }

    // STILL CHARGING. One frame of charge per frame of hold, capped, and at the cap she leaves for
    // SPECIAL_N_E rather than sitting in the loop -- which is what makes "fully charged" a visible
    // state rather than an invisible number.
    let entry = entry_of(boma);
    state::inc_count(entry);
    let cap = charge_frame(boma);
    if state::count(entry) as f32 >= cap {
        state::set_count(entry, cap as i32);
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_N_E.into(), false.into());
        return 1.into();
    }
    0.into()
}

unsafe extern "C" fn special_n_h_end(_fighter: &mut L2CFighterCommon) -> L2CValue {
    0.into()
}

/// `SpecialNH_exit`, samus.
///
///     next = <status being changed to>
///     if next is SPECIAL_N_F, _E, _C, _JUMP_CANCEL, GUARD_ON, ESCAPE, ESCAPE_F or ESCAPE_B then
///         ArticleModule::remove_exist(CSHOT, 0)
///         if <the charge is at the cap> then <request charge_max, followed on `armr`> end
///     else
///         <abandon: zero the charge, remove the article, remove the effect>
///     end
///
/// THE ARTICLE IS REMOVED ON BOTH PATHS AND THE CHARGE ONLY ON ONE. That asymmetry is the rule the
/// whole move runs on: the projectile is re-generated by the ACMD every time the windup plays, so
/// it is disposable; the charge is the thing worth keeping, and it is kept for every DELIBERATE
/// exit -- including shielding and rolling out of a charge, which is exactly how the move is used.
unsafe extern "C" fn special_n_h_exit(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let next = StatusModule::status_kind_next(boma);
    let deliberate = next == RAT_STATUS_KIND_SPECIAL_N_F
        || next == RAT_STATUS_KIND_SPECIAL_N_E
        || next == RAT_STATUS_KIND_SPECIAL_N_C
        || next == RAT_STATUS_KIND_SPECIAL_N_JUMP_CANCEL
        || next == *FIGHTER_STATUS_KIND_GUARD_ON
        || next == *FIGHTER_STATUS_KIND_ESCAPE
        || next == *FIGHTER_STATUS_KIND_ESCAPE_F
        || next == *FIGHTER_STATUS_KIND_ESCAPE_B;
    if deliberate {
        remove_cshot(boma);
        // The `charge_max` glow would be requested here. See the header: it is one of samus's own
        // effects and ADR 0002 keeps Pikachu's archives, so there is nothing to request.
    } else {
        abandon_charge(fighter);
    }
    0.into()
}

// ================================================================================================
// SPECIAL_N_C and SPECIAL_N_E -- the cancel, and the cap
// ================================================================================================

/// `SpecialNC_main` / `SpecialNE_main`, samus. Both play the cancel animation from frame 0.
unsafe extern "C" fn special_n_c_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    ground_air_setup(
        boma,
        on_ground(fighter),
        "special_n_c",
        "special_air_n_c",
        *GROUND_CORRECT_KIND_GROUND_CLIFF_STOP,
        true,
    );
    ControlModule::set_add_jump_mini_button_life(boma, 8);
    fighter.sub_shift_status_main(L2CValue::Ptr(special_n_c_main_loop as *const () as _))
}

/// `SpecialNC_main_loop`, samus.
///
///     sub_check_charge_cancel_jump_mini_attack()
///     if CANCEL_TYPE == GROUND_GUARD and not check_button_on(GUARD) then CANCEL_TYPE = NONE end
///     if on ground and check_button_trigger(ATTACK) and CANCEL_TYPE == GROUND_JUMP then
///         FighterControlModuleImpl::update_attack_air_kind()
///     end
///     <situation swap; if it left a status change pending, CANCEL_TYPE = NONE and stop>
///     if CANCEL_TYPE == NONE then done = MotionModule::is_end()
///                            else done = MotionModule::is_end() or is_enable_cancel() end
///     if done then <hand off by cancel type> end
///     if is_enable_cancel() then
///         if sub_wait_ground_check_common(false) then return 1 end
///         if sub_air_check_fall_common()         then return 1 end
///     end
unsafe extern "C" fn special_n_c_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    fighter.sub_check_charge_cancel_jump_mini_attack();

    let mut ty = WorkModule::get_int(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_WORK_INT_CANCEL_TYPE);
    if ty == *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_GROUND_GUARD
        && !ControlModule::check_button_on(boma, *CONTROL_PAD_BUTTON_GUARD)
    {
        ty = *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_NONE;
        WorkModule::set_int(boma, ty, *FIGHTER_SAMUS_STATUS_SPECIAL_N_WORK_INT_CANCEL_TYPE);
    }
    if on_ground(fighter)
        && ControlModule::check_button_trigger(boma, *CONTROL_PAD_BUTTON_ATTACK)
        && ty == *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_GROUND_JUMP
    {
        FighterControlModuleImpl::update_attack_air_kind(boma, false);
    }

    situation_swap(
        fighter,
        "special_n_c",
        "special_air_n_c",
        *GROUND_CORRECT_KIND_GROUND_CLIFF_STOP,
    );
    if StatusModule::is_changing(boma) {
        WorkModule::set_int(
            boma,
            *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_NONE,
            *FIGHTER_SAMUS_STATUS_SPECIAL_N_WORK_INT_CANCEL_TYPE,
        );
        return 0.into();
    }

    let cancellable = CancelModule::is_enable_cancel(boma);
    let done = if ty == *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_NONE {
        MotionModule::is_end(boma)
    } else {
        MotionModule::is_end(boma) || cancellable
    };
    if done {
        return cancel_handoff(fighter, ty);
    }
    if cancellable {
        if fighter.sub_wait_ground_check_common(false.into()).get_bool() {
            return 1.into();
        }
        if fighter.sub_air_check_fall_common().get_bool() {
            return 1.into();
        }
    }
    0.into()
}

/// Where a finished cancel goes, by cancel type.
///
/// THE ONE PIECE OF THIS MOVE THAT IS DERIVED RATHER THAN TRANSCRIBED, and it is marked as such.
/// Her `SpecialNC_main_loop` fastshifts to an unnamed static, and the seven cancel types are the
/// only input that static has -- each names a status the engine already has a transition term for,
/// and `SpecialNH_main` enabled exactly those terms. `NONE` falls through to `WAIT` / `FALL` by the
/// situation, which is what "the animation finished and nothing was asked for" means everywhere
/// else in the game.
unsafe fn cancel_handoff(fighter: &mut L2CFighterCommon, ty: i32) -> L2CValue {
    let status = if ty == *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_GROUND_ESCAPE {
        *FIGHTER_STATUS_KIND_ESCAPE
    } else if ty == *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_GROUND_ESCAPE_F {
        *FIGHTER_STATUS_KIND_ESCAPE_F
    } else if ty == *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_GROUND_ESCAPE_B {
        *FIGHTER_STATUS_KIND_ESCAPE_B
    } else if ty == *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_GROUND_GUARD {
        *FIGHTER_STATUS_KIND_GUARD_ON
    } else if ty == *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_GROUND_JUMP {
        *FIGHTER_STATUS_KIND_JUMP_SQUAT
    } else if ty == *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_AIR_ESCAPE_AIR {
        *FIGHTER_STATUS_KIND_ESCAPE_AIR
    } else if ty == *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_AIR_JUMP_AERIAL {
        *FIGHTER_STATUS_KIND_JUMP_AERIAL
    } else if on_ground(fighter) {
        *FIGHTER_STATUS_KIND_WAIT
    } else {
        *FIGHTER_STATUS_KIND_FALL
    };
    fighter.change_status(status.into(), false.into());
    1.into()
}

/// `SpecialNC_end`, samus:
///
///     WorkModule::set_int(get_param_int(param_special_n, <unlabelled>),
///                         FIGHTER_INSTANCE_WORK_ID_INT_AIR_ESCAPE_ADD_XLU_START_FRAME)
///
/// The unlabelled key is 0 in her prc, so this hands the air dodge no extra intangibility start-up.
/// Transcribed anyway rather than folded to a literal 0: the read goes through the param provider,
/// so if the number ever stops being 0 this follows it.
unsafe extern "C" fn special_n_c_end(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let v = WorkModule::get_param_int(boma, PARAM_SPECIAL_N, CSHOT_AIR_ESCAPE_ADD_XLU_START_FRAME);
    WorkModule::set_int(boma, v, *FIGHTER_INSTANCE_WORK_ID_INT_AIR_ESCAPE_ADD_XLU_START_FRAME);
    0.into()
}

// ================================================================================================
// SPECIAL_N_JUMP_CANCEL -- the air jump out of a charge
// ================================================================================================

/// `SpecialNJumpCancel_main`, samus. Air only -- the status is only ever entered from the air
/// branch of the charge loop -- so there is no ground/air split here.
unsafe extern "C" fn special_n_jump_cancel_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
    KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_AIR_STOP);
    MotionModule::change_motion(
        boma,
        Hash40::new("special_air_n_jump_c"),
        0.0,
        1.0,
        false,
        0.0,
        false,
        false,
    );
    fighter.sub_shift_status_main(L2CValue::Ptr(special_n_jump_cancel_main_loop as *const () as _))
}

/// `SpecialNJumpCancel_main_loop`, samus.
///
///     if MotionModule::is_end() then
///         if CANCEL_TYPE == AIR_JUMP_AERIAL then
///             if get_param_int(aerial_type, 0) == FIGHTER_JUMP_AERIAL_TYPE_NORMAL
///                 then change_status(JUMP_AERIAL) else change_status(FLY) end
///         else
///             change_status(FALL)
///         end
///         return 1
///     end
///     if globalTable[0x17] == AIR and globalTable[0x16] == GROUND then
///         change_status(LANDING); return 1
///     end
unsafe extern "C" fn special_n_jump_cancel_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    if MotionModule::is_end(boma) {
        let ty = WorkModule::get_int(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_WORK_INT_CANCEL_TYPE);
        let next = if ty != *FIGHTER_SAMUS_SPECIAL_N_CANCEL_TYPE_AIR_JUMP_AERIAL {
            *FIGHTER_STATUS_KIND_FALL
        } else if WorkModule::get_param_int(boma, AERIAL_TYPE, 0) == *FIGHTER_JUMP_AERIAL_TYPE_NORMAL
        {
            *FIGHTER_STATUS_KIND_JUMP_AERIAL
        } else {
            *FIGHTER_STATUS_KIND_FLY
        };
        fighter.change_status(next.into(), false.into());
        return 1.into();
    }
    if fighter.global_table[GT_SITUATION_PREV].get_i32() == *SITUATION_KIND_AIR
        && fighter.global_table[GT_SITUATION].get_i32() == *SITUATION_KIND_GROUND
    {
        fighter.change_status((*FIGHTER_STATUS_KIND_LANDING).into(), false.into());
        return 1.into();
    }
    0.into()
}

// ================================================================================================
// SPECIAL_N_F -- the shot
// ================================================================================================

/// `SpecialNF_init`, samus.
///
///     WorkModule::set_int(get_int(INSTANCE_INT_SPECIAL_N_COUNT), STATUS_SPECIAL_N_WORK_INT_COUNT)
///     WorkModule::set_int(0, INSTANCE_INT_SPECIAL_N_COUNT)
///     EffectModule::remove_common(hash40("charge_max"))
///     <MA_MSC_EFFECT_REMOVE on EFH_CHARGE_MAX>
///
/// THE CHARGE IS SPENT HERE, not when the projectile leaves. Moving it into status work is what
/// lets `exec` and `main` read how big the shot is on the frames that need it while the stored
/// charge is already back to zero -- so being interrupted after this point cannot refund it.
unsafe extern "C" fn special_n_f_init(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    WorkModule::set_int(
        boma,
        state::count(entry),
        *FIGHTER_SAMUS_STATUS_SPECIAL_N_WORK_INT_COUNT,
    );
    state::set_count(entry, 0);
    0.into()
}

/// `SpecialNF_exec`, samus.
///
///     if WorkModule::is_flag(FLAG_SHOOT) then
///         ratio = STATUS_SPECIAL_N_WORK_INT_COUNT
///                 / get_param_float(param_special_n, cshot_charge_frame)
///         spd   = lerp(cshot_shot_spd_min, cshot_shot_spd_max, ratio) * PostureModule::lr()
///         sv_kinetic_energy::set_speed(FIGHTER_KINETIC_ENERGY_ID_STOP, spd,
///                                      <the STOP energy's current y>)
///         WorkModule::off_flag(FLAG_SHOOT)
///     end
///
/// THE RECOIL, AND IT IS THE ONLY THING IN THE MOVE THAT SCALES CONTINUOUSLY WITH CHARGE. `min` is
/// -0.1 and `max` -0.8 in her prc, both NEGATIVE and both multiplied by `lr`, so she is pushed
/// BACKWARDS -- eight times harder from a full charge than from a tap. `FLAG_SHOOT` is raised on
/// the same frame the article is fired, which is how the push lands on the frame the shot leaves
/// rather than on the frame the status began.
unsafe extern "C" fn special_n_f_exec(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    if !WorkModule::is_flag(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_SHOOT) {
        return 0.into();
    }
    let count = WorkModule::get_int(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_WORK_INT_COUNT) as f32;
    let cap = charge_frame(boma);
    let ratio = if cap > 0.0 { (count / cap).min(1.0) } else { 0.0 };
    let min = WorkModule::get_param_float(boma, PARAM_SPECIAL_N, CSHOT_SHOT_SPD_MIN);
    let max = WorkModule::get_param_float(boma, PARAM_SPECIAL_N, CSHOT_SHOT_SPD_MAX);
    let speed_x = (min + (max - min) * ratio) * PostureModule::lr(boma);

    // The y component is READ BACK OFF THE ENERGY rather than zeroed. `set_speed` takes both
    // components, and passing 0 would delete whatever vertical the STOP energy was holding --
    // which in the air is the difference between a recoil and a snap to a standstill.
    fighter.clear_lua_stack();
    fighter.push_lua_stack(&mut L2CValue::new_int(*FIGHTER_KINETIC_ENERGY_ID_STOP as u64));
    let speed_y = sv_kinetic_energy::get_speed_y(fighter.lua_state_agent);

    fighter.clear_lua_stack();
    fighter.push_lua_stack(&mut L2CValue::new_int(*FIGHTER_KINETIC_ENERGY_ID_STOP as u64));
    fighter.push_lua_stack(&mut L2CValue::new_num(speed_x));
    fighter.push_lua_stack(&mut L2CValue::new_num(speed_y));
    sv_kinetic_energy::set_speed(fighter.lua_state_agent);

    WorkModule::off_flag(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_SHOOT);
    0.into()
}

/// Which of the four shot motions this shot uses: the MAX variant only at a full charge.
unsafe fn shot_motion(fighter: &mut L2CFighterCommon) -> &'static str {
    let boma = fighter.module_accessor;
    let count = WorkModule::get_int(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_WORK_INT_COUNT) as f32;
    let maxed = count >= charge_frame(boma);
    match (on_ground(fighter), maxed) {
        (true, false) => "special_n_f",
        (true, true) => "special_n_f_max",
        (false, false) => "special_air_n_f",
        (false, true) => "special_air_n_f_max",
    }
}

/// `SpecialNF_main`, samus.
unsafe extern "C" fn special_n_f_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let motion = shot_motion(fighter);
    if WorkModule::is_flag(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_MOT_RESTART) {
        MotionModule::change_motion(boma, Hash40::new(motion), 0.0, 1.0, false, 0.0, false, false);
        WorkModule::off_flag(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_MOT_RESTART);
    } else {
        // The charge loop already changed the motion on the frame the button came up, so this
        // inherits that frame instead of restarting and losing it.
        MotionModule::change_motion_inherit_frame(
            boma, Hash40::new(motion), -1.0, 1.0, 0.0, false, false,
        );
    }
    if on_ground(fighter) {
        GroundModule::correct(
            boma,
            GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP_ATTACK),
        );
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_GROUND_STOP);
    } else {
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_AIR_STOP);
    }
    // Her `main` zeroes the stored charge a SECOND time, after `init` already did. Transcribed
    // rather than tidied away: the two are reached independently -- a stored full charge enters
    // this status straight from the windup -- and the duplicate is the source's own belt and
    // braces.
    state::set_count(entry_of(boma), 0);
    fighter.sub_shift_status_main(L2CValue::Ptr(special_n_f_main_loop as *const () as _))
}

/// `SpecialNF_main_loop`, samus. Once the shot is cancellable the ordinary ground/fall checks take
/// over; until then it is the situation swap and "end when the animation does".
unsafe extern "C" fn special_n_f_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    if CancelModule::is_enable_cancel(boma) {
        if fighter.sub_wait_ground_check_common(false.into()).get_bool() {
            return 1.into();
        }
        if fighter.sub_air_check_fall_common().get_bool() {
            return 1.into();
        }
    }
    // The situation swap has to name the motion pair the CURRENT charge level chose, or landing
    // mid-shot would drop a max-charge animation back to the ordinary one.
    let maxed = shot_motion(fighter).ends_with("_max");
    let (ground, air) = if maxed {
        ("special_n_f_max", "special_air_n_f_max")
    } else {
        ("special_n_f", "special_air_n_f")
    };
    situation_swap(fighter, ground, air, *GROUND_CORRECT_KIND_GROUND_CLIFF_STOP_ATTACK);
    if !MotionModule::is_end(boma) {
        return 0.into();
    }
    // Her loop fastshifts to the common end-of-attack handoff, and
    // `sub_wait_ground_check_common` / `sub_air_check_fall_common` ARE that handoff for a grounded
    // and an airborne fighter -- so they are called unconditionally once the animation is over,
    // rather than only inside the cancel window.
    if fighter.sub_wait_ground_check_common(false.into()).get_bool() {
        return 1.into();
    }
    if fighter.sub_air_check_fall_common().get_bool() {
        return 1.into();
    }
    0.into()
}

unsafe extern "C" fn special_n_f_end(_fighter: &mut L2CFighterCommon) -> L2CValue {
    0.into()
}

/// `SpecialNF_exit`, samus: the charge is already spent, so this only tears the article down.
unsafe extern "C" fn special_n_f_exit(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    state::set_count(entry_of(boma), 0);
    remove_cshot(boma);
    0.into()
}

// ================================================================================================
// The two article calls the ACMD is not allowed to make
// ================================================================================================

/// Generate the charge shot. Called from the windup's ACMD frame by `src/specials_acmd`.
///
/// `tools/reach/src/bin/deforeign.rs` strips
/// `ArticleModule::generate_article_enable(FIGHTER_SAMUS_GENERATE_ARTICLE_CSHOT, ...)` out of the
/// generated script, correctly: that constant is 0 and 0 is Pikachu's electro ball. This is the
/// same call with the id the clone actually got.
pub unsafe fn generate(boma: *mut BattleObjectModuleAccessor) {
    let Some(id) = cshot() else { return };
    ArticleModule::generate_article_enable(boma, id, false, -1);
    WorkModule::on_flag(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_BULLET_DISP);
}

/// Fire it. Same reasoning as `generate`.
pub unsafe fn shoot(boma: *mut BattleObjectModuleAccessor) {
    let Some(id) = cshot() else { return };
    ArticleModule::shoot_exist(boma, id, ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL), false);
    WorkModule::on_flag(boma, *FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_SHOOT);
}

/// Whether Charge Shot is installed at all.
///
/// A DECLARED SWITCH RATHER THAN COMMENTED-OUT CALLS, for the same reason `CLONE_ENABLED` in
/// src/articles is one: the build after this move landed crashed loading into training, and THREE
/// things in it can crash at fighter load -- two newly cloned articles and these five new status
/// kinds, which move `RAT_STATUS_KIND_NUM` from 0x207 to 0x20C and are what smashline sizes the
/// fighter's status table from. Bisecting that needs each to be one line and one build, not a
/// revert; the articles already had their switch and this is the one they were missing.
///
/// With it false nothing here registers, `special_n` stays Pikachu's Thunder Jolt, and the eleven
/// synthesised motions sit unused in the motion list -- which is inert, not harmful.
pub const CHARGESHOT_ENABLED: bool = true;

pub fn install() {
    if !CHARGESHOT_ENABLED {
        crate::telemetry::sink::diag(
            "[rat] chargeshot: CHARGESHOT_ENABLED=false, neutral-B is Pikachu's own".into(),
        );
        return;
    }
    acmd::install();
    Agent::new("pikachu")
        .status(Pre, *FIGHTER_STATUS_KIND_SPECIAL_N, special_n_pre)
        .status(Init, *FIGHTER_STATUS_KIND_SPECIAL_N, special_n_init)
        .status(Main, *FIGHTER_STATUS_KIND_SPECIAL_N, special_n_main)
        .status(End, *FIGHTER_STATUS_KIND_SPECIAL_N, special_n_end)
        .status(Exit, *FIGHTER_STATUS_KIND_SPECIAL_N, special_n_exit)
        .status(Pre, RAT_STATUS_KIND_SPECIAL_N_H, special_n_h_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_N_H, special_n_h_main)
        .status(End, RAT_STATUS_KIND_SPECIAL_N_H, special_n_h_end)
        .status(Exit, RAT_STATUS_KIND_SPECIAL_N_H, special_n_h_exit)
        .status(Pre, RAT_STATUS_KIND_SPECIAL_N_C, special_n_h_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_N_C, special_n_c_main)
        // `Exit`, NOT `End`, AND THAT IS NOT A TRANSCRIPTION SLIP. Samus registers this body as
        // her status's END line -- but END DOES NOT FIRE FOR A STATUS KIND THAT IS NOT THE
        // FIGHTER'S OWN, which is what `tools/check-statuslines.sh` exists to catch and what it
        // caught here. Registered as End it would never have run, and the air dodge out of a
        // cancel would have kept whatever `AIR_ESCAPE_ADD_XLU_START_FRAME` the previous move
        // left. Revenge's retaliation paid for this same lesson; see that gate's header.
        .status(Exit, RAT_STATUS_KIND_SPECIAL_N_C, special_n_c_end)
        // SPECIAL_N_E IS SPECIAL_N_C WITH A DIFFERENT NAME, and her two status trees really are
        // the same functions: `SpecialNE_pre`, `_main` and `_main_loop` are character-for-character
        // `SpecialNC`'s, down to using the `special_n_c` motions. It is a separate status only so
        // that reaching the charge cap is distinguishable from cancelling -- the charge is kept
        // either way, but `SpecialNH_exit` tests for both by name.
        .status(Pre, RAT_STATUS_KIND_SPECIAL_N_E, special_n_h_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_N_E, special_n_c_main)
        .status(Exit, RAT_STATUS_KIND_SPECIAL_N_E, special_n_c_end)
        .status(Pre, RAT_STATUS_KIND_SPECIAL_N_JUMP_CANCEL, special_n_h_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_N_JUMP_CANCEL, special_n_jump_cancel_main)
        // NO `End` AND NO `Exit`. `SpecialNJumpCancel_end` is a bare `return 0` in her tree, so
        // there is no body to run -- and registering End on a new status kind, where End does not
        // fire, would be a registration that asserts something untrue about itself.
        .status(Pre, RAT_STATUS_KIND_SPECIAL_N_F, special_n_h_pre)
        .status(Init, RAT_STATUS_KIND_SPECIAL_N_F, special_n_f_init)
        .status(Exec, RAT_STATUS_KIND_SPECIAL_N_F, special_n_f_exec)
        .status(Main, RAT_STATUS_KIND_SPECIAL_N_F, special_n_f_main)
        .status(End, RAT_STATUS_KIND_SPECIAL_N_F, special_n_f_end)
        .status(Exit, RAT_STATUS_KIND_SPECIAL_N_F, special_n_f_exit)
        .install();
}
