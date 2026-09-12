// Alolan Whip, transcribed from Incineroar's own status functions.
//
// THIS IS A REWRITE, AND ADR 0006 SAYS WHY. The module this replaces was reimplemented from a
// description of what the move does and then patched across seven playtest rounds, each of which
// fixed something real and left the move broken a new way. Nothing here is written from a
// description. Every function below is a transcription of a specific decompiled function at a
// named address, and the source is quoted above each one so the two can be read side by side.
//
// WHERE THE SOURCE COMES FROM, so it can be regenerated rather than trusted:
//
//     bash tools/disasm/target/release/disasm.exe \
//       vendor/nro/prebuilt/nro/release/lua2cpp_gaogaen.nro --statusmap 0x7100000480
//
// recovers all 98 of Incineroar's status functions with their status kind and line, because the
// NRO exports exactly one symbol for its status scripts and every function is an unnamed static.
// Then `bash tools/decomp.sh gaogaen <addr>` decompiles one to annotated C.
//
//     status                         line   address
//     FIGHTER_STATUS_KIND_SPECIAL_S  PRE    0x7100003c90
//     FIGHTER_STATUS_KIND_SPECIAL_S  MAIN   0x7100004070
//     ..._SPECIAL_S_THROW            PRE    0x71000044e0
//     ..._SPECIAL_S_THROW            MAIN   0x71000048c0
//     ..._SPECIAL_S_THROW            END    0x7100004ba0
//     ..._SPECIAL_S_SHOULDER         PRE    0x7100005170
//     ..._SPECIAL_S_LARIAT           PRE    0x7100005870
//     ..._SPECIAL_S_FAILURE          PRE    0x7100006310
//     ..._SPECIAL_S_END              PRE    0x7100006b70
//
// NOT REGISTERED YET. A half-transcribed move sharing its statuses with a half-patched one is
// worse than either, so `install` is not called from `specials::install` until every status here
// is transcribed. The old `specials/specials/mod.rs` still owns the move in the shipped build.
pub mod state;

use super::status::*;
use {
    smash::{
        lua2cpp::*,
        phx::*,
        app::{lua_bind::*, KineticUtility, *},
        lib::{lua_const::*, L2CValue},
    },
    smashline::{*, Priority::*},
};

unsafe fn entry_of(boma: *mut BattleObjectModuleAccessor) -> usize {
    WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize
}

/// The `pre` line every one of the six whip statuses shares, argument for argument.
///
/// SOURCE, `FIGHTER_STATUS_KIND_SPECIAL_S` PRE at gaogaen 0x7100003c90:
///
///     StatusModule::init_settings(boma, SITUATION_KIND_NONE, FIGHTER_KINETIC_TYPE_UNIQ,
///         GROUND_CORRECT_KIND_KEEP, GROUND_CLIFF_CHECK_KIND_NONE, true,
///         FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLAG, ..._NONE_INT, ..._NONE_FLOAT, 0)
///     FighterStatusModuleImpl::set_fighter_status_data(boma, false,
///         FIGHTER_TREADED_KIND_NO_REAC, false, false, false,
///         FIGHTER_LOG_MASK_FLAG_ACTION_CATEGORY_ATTACK
///             | FIGHTER_LOG_MASK_FLAG_ATTACK_KIND_SPECIAL_S
///             | FIGHTER_LOG_MASK_FLAG_ACTION_TRIGGER_ON,
///         <status attr>, FIGHTER_POWER_UP_ATTACK_BIT_SPECIAL_S, 0)
///
/// THE KINETIC TYPE HERE IS UNIQ, AND IT DOES NOT SURVIVE THE FRAME. `init_settings` declares
/// FIGHTER_KINETIC_TYPE_UNIQ for all six of these statuses, and then `whip_kinetics_enter`
/// immediately overrides it with GROUND_STOP on the ground or FALL in the air -- which is not
/// visible in this function at all, and was only established by recording Incineroar playing the
/// move. Transcribing this `pre` faithfully and stopping there would have produced a move on the
/// wrong kinetic type, which is worth remembering the next time a status looks fully understood.
///
/// Only the status attribute differs between the six, so it is the one parameter: the catch passes
/// FIGHTER_STATUS_ATTR_START_TURN and the throw passes FIGHTER_STATUS_ATTR_DISABLE_TURN_DAMAGE.
unsafe fn whip_pre(fighter: &mut L2CFighterCommon, status_attr: i32) -> L2CValue {
    let boma = fighter.module_accessor;
    StatusModule::init_settings(
        boma,
        SituationKind(*SITUATION_KIND_NONE),
        *FIGHTER_KINETIC_TYPE_UNIQ,
        *GROUND_CORRECT_KIND_KEEP as u32,
        GroundCliffCheckKind(*GROUND_CLIFF_CHECK_KIND_NONE),
        true,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLAG,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_INT,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLOAT,
        0,
    );
    FighterStatusModuleImpl::set_fighter_status_data(
        boma,
        false,
        *FIGHTER_TREADED_KIND_NO_REAC,
        false,
        false,
        false,
        (*FIGHTER_LOG_MASK_FLAG_ACTION_CATEGORY_ATTACK
            | *FIGHTER_LOG_MASK_FLAG_ATTACK_KIND_SPECIAL_S
            | *FIGHTER_LOG_MASK_FLAG_ACTION_TRIGGER_ON) as u64,
        status_attr as u32,
        *FIGHTER_POWER_UP_ATTACK_BIT_SPECIAL_S as u32,
        0,
    );
    0.into()
}

/// The whip's kinetic setup, transcribed from the RECORDED sequence rather than the source tree.
///
/// WHY A RECORDING AND NOT THE SOURCE, since ADR 0006 says transcribe. The source exists: every
/// one of Incineroar's six side-B statuses calls `FUN_710001d4b0` at gaogaen 0x710001d4b0, which
/// is 62 kinetic calls over 748 lines of decompiled C. But it dispatches on the current status
/// through a six-deep nest of inverted conditions and then on the situation and on a bool
/// argument, and it contains TWO `change_kinetic(GROUND_STOP)` sites in different branches that
/// produce an identical observable. Reading which one runs out of that is not transcription, it is
/// reconstruction, and a reconstruction is what the module this replaces was made of.
///
/// So the branch was measured instead. `src/telemetry/enginetrace.rs` hooks change_kinetic,
/// enable_energy and clear_unable_energy and tags each call with the status and motion it came
/// from; `KineticUtility::clear_unable_energy` is the decisive one because it passes its argument
/// directly rather than through the lua stack. Incineroar's own side-B, on the ground and in the
/// air, then prints its kinetic setup in execution order:
///
///   GROUND, mot=special_s_start                 AIR, mot=special_air_s_start
///   ----------------------------------          ----------------------------------
///   change_kinetic(GROUND_STOP)                 change_kinetic(FALL)
///                                               enable_energy(STOP)
///   clear_unable_energy(MOTION)                 clear_unable_energy(MOTION)
///   clear_unable_energy(CONTROL)                clear_unable_energy(CONTROL)
///   enable_energy(MOTION)                       enable_energy(MOTION)
///   clear_unable_energy(GRAVITY)                clear_unable_energy(GRAVITY)
///   clear_unable_energy(CONTROL)                clear_unable_energy(CONTROL)
///   clear_unable_energy(STOP)                   clear_unable_energy(STOP)
///
/// identical across every repetition, and the air path is the ground path with two calls swapped
/// in at the front. This function IS that table.
///
/// AND IT CORRECTS SOMETHING THE OLD MODULE HAD WRONG IN EVERY STATUS. `init_settings` declares
/// FIGHTER_KINETIC_TYPE_UNIQ for all six of these statuses -- that much was transcribed from the
/// source correctly -- and then this setup immediately overrides it with GROUND_STOP on the ground
/// and FALL in the air. The old module passed MOTION for some statuses and GROUND_STOP for others,
/// reasoning about which one "should" carry him, and never enabled the STOP energy at all. That is
/// most of what "the timing is messed up" was.
///
/// `sv_kinetic_energy::reset_energy` and `set_speed_mul` are NOT here. They are called through the
/// lua stack, so the tracer cannot see them, and their arguments in the matching static block
/// depend on which block ran -- the exact thing that could not be established. They are the
/// remaining gap in this function and it is named rather than guessed: see the module header.
unsafe fn whip_kinetics_enter(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;

    if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_GROUND_STOP);
    } else {
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_FALL);
        KineticModule::enable_energy(boma, *FIGHTER_KINETIC_ENERGY_ID_STOP);
    }

    KineticUtility::clear_unable_energy(*FIGHTER_KINETIC_ENERGY_ID_MOTION, boma);
    KineticUtility::clear_unable_energy(*FIGHTER_KINETIC_ENERGY_ID_CONTROL, boma);
    KineticModule::enable_energy(boma, *FIGHTER_KINETIC_ENERGY_ID_MOTION);
    KineticUtility::clear_unable_energy(*FIGHTER_KINETIC_ENERGY_ID_GRAVITY, boma);
    KineticUtility::clear_unable_energy(*FIGHTER_KINETIC_ENERGY_ID_CONTROL, boma);
    KineticUtility::clear_unable_energy(*FIGHTER_KINETIC_ENERGY_ID_STOP, boma);
}

/// The same, for SPECIAL_S_THROW. Recorded, `mot=special_s_throw` and `special_air_s_throw` alike:
///
///   clear_unable_energy(MOTION)
///   clear_unable_energy(GRAVITY)
///   clear_unable_energy(CONTROL)
///   enable_energy(STOP)
///
/// NO `change_kinetic` AT ALL, on the ground or in the air -- the throw inherits whatever the
/// catch left, which is why its `init_settings` passes GROUND_CORRECT_KIND_KEEP.
unsafe fn whip_kinetics_throw(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    KineticUtility::clear_unable_energy(*FIGHTER_KINETIC_ENERGY_ID_MOTION, boma);
    KineticUtility::clear_unable_energy(*FIGHTER_KINETIC_ENERGY_ID_GRAVITY, boma);
    KineticUtility::clear_unable_energy(*FIGHTER_KINETIC_ENERGY_ID_CONTROL, boma);
    KineticModule::enable_energy(boma, *FIGHTER_KINETIC_ENERGY_ID_STOP);
}

/// The follow-ups -- SHOULDER, LARIAT and FAILURE. Recorded, and identical for all three:
///
///   enable_energy(MOTION)
///   clear_unable_energy(GRAVITY)
///   clear_unable_energy(CONTROL)
///   clear_unable_energy(STOP)
#[allow(dead_code)]
unsafe fn whip_kinetics_followup(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    KineticModule::enable_energy(boma, *FIGHTER_KINETIC_ENERGY_ID_MOTION);
    KineticUtility::clear_unable_energy(*FIGHTER_KINETIC_ENERGY_ID_GRAVITY, boma);
    KineticUtility::clear_unable_energy(*FIGHTER_KINETIC_ENERGY_ID_CONTROL, boma);
    KineticUtility::clear_unable_energy(*FIGHTER_KINETIC_ENERGY_ID_STOP, boma);
}

unsafe extern "C" fn special_s_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    whip_pre(fighter, *FIGHTER_STATUS_ATTR_START_TURN)
}

/// SOURCE, `FIGHTER_STATUS_KIND_SPECIAL_S` MAIN at gaogaen 0x7100004070:
///
///     WorkModule::set_int(boma, BATTLE_OBJECT_ID_INVALID,
///         FIGHTER_GAOGAEN_INSTANCE_WORK_ID_INT_BATTLE_OBJECT_ID_SWING_THROWN_FIGHTER)
///     WorkModule::on_flag(boma, FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLAG_INVALID_SPECIAL_AIR_S)
///     if globalTable[0x16] == SITUATION_KIND_GROUND:
///         GroundModule::correct(GROUND_CORRECT_KIND_GROUND_CLIFF_STOP)
///         MotionModule::change_motion(hash40("special_s_start"), 0.0, 1.0, false, 0.0, false, false)
///     else:
///         GroundModule::correct(GROUND_CORRECT_KIND_AIR)
///         MotionModule::change_motion(hash40("special_air_s_start"), 0.0, 1.0, false, 0.0, false, false)
///     FUN_710001d4b0(this, ...)
///     if !StopModule::is_stop(boma):
///         FUN_7100023730(this)
///     GroundModule::select_cliff_hangdata(boma, FIGHTER_GAOGAEN_CLIFF_HANG_DATA_SPECIAL_S)
///     sub_shift_status_main(0x7100023970)
///
/// THE TWO WORK WRITES GO THROUGH `state`, NOT `WorkModule`, and that is the one deliberate
/// deviation in this function. `FLAG_INVALID_SPECIAL_AIR_S` is 0x200000E7 and
/// `FIGHTER_PIKACHU_INSTANCE_WORK_ID_FLAG_PAUSE` is 0x200000E7 -- the same slot, because every
/// fighter numbers its own instance work from the end of the common block. Writing Incineroar's
/// flag on a Pikachu silently sets FLAG_PAUSE. See state.rs and ADR 0006.
///
/// `select_cliff_hangdata` takes a GAOGAEN cliff-hang-data id, which is the same class of
/// per-fighter constant, so it is not called: Pikachu keeps whatever ledge data his own status
/// left, and the observable is how the whip behaves at a ledge.
unsafe extern "C" fn special_s_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);

    state::reset(entry);
    state::set_swing_thrown_fighter(entry, *BATTLE_OBJECT_ID_INVALID as u32);
    state::set_invalid_special_air_s(entry, true);

    if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
        MotionModule::change_motion(
            boma, Hash40::new("special_s_start"), 0.0, 1.0, false, 0.0, false, false,
        );
    } else {
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
        MotionModule::change_motion(
            boma, Hash40::new("special_air_s_start"), 0.0, 1.0, false, 0.0, false, false,
        );
    }

    whip_kinetics_enter(fighter);

    fighter.sub_shift_status_main(L2CValue::Ptr(special_s_main_loop as *const () as _))
}

/// SOURCE, the loop `SPECIAL_S` MAIN shifts to, gaogaen 0x7100023970 -> FUN_7100023730:
///
///     if <not stopped>:
///         if WorkModule::is_flag(A) { off_flag(A); on_flag(B); FUN_710001d4b0(this) }
///         if WorkModule::is_flag(C) { off_flag(C); off_flag(D); FUN_710001d4b0(this) }
///
/// STILL BEING TRANSCRIBED. The four flags and `FUN_710001d4b0` are the move's own movement
/// handling -- the helper reads FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_MOVE_START and
/// KINETIC_ENERGY_RESERVE_ATTRIBUTE_MAIN -- and transcribing it needs its own pass. Until then
/// this loop does nothing, which is honest: an empty transcription is visibly incomplete, whereas
/// a guess at what those flags mean is what the old module was made of.
unsafe extern "C" fn special_s_main_loop(_fighter: &mut L2CFighterCommon) -> L2CValue {
    0.into()
}

unsafe extern "C" fn special_s_throw_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    whip_pre(fighter, *FIGHTER_STATUS_ATTR_DISABLE_TURN_DAMAGE)
}

/// SOURCE, `FIGHTER_GAOGAEN_STATUS_KIND_SPECIAL_S_THROW` MAIN at gaogaen 0x71000048c0:
///
///     FighterSpecializer_Gaogaen::generate_rope(boma)
///     if globalTable[0x16] == SITUATION_KIND_GROUND:
///         GroundModule::correct(GROUND_CORRECT_KIND_GROUND_CLIFF_STOP)
///         MotionModule::change_motion(hash40("special_s_throw"), 0.0, 1.0, false, 0.0, false, false)
///     else:
///         GroundModule::correct(GROUND_CORRECT_KIND_AIR)
///         MotionModule::change_motion(hash40("special_air_s_throw"), 0.0, 1.0, false, 0.0, false, false)
///     FUN_710001d4b0(this, true)
///     sub_shift_status_main(<the throw's loop>)
///
/// `generate_rope` IS CALLED, exported and by name, because that is what the source does and
/// because whether it produces a rope for a Pikachu is a measurement rather than an assumption --
/// `ArticleModule::is_exist` before and after says so. ADR 0004 covers what happens if it does
/// not: the victim's own status reads
/// FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLAG_SPECIAL_S_GENERATED_ROPE off the captor and takes the
/// no-rope path.
unsafe extern "C" fn special_s_throw_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;

    smash::app::FighterSpecializer_Gaogaen::generate_rope(boma as *mut FighterModuleAccessor);

    if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
        MotionModule::change_motion(
            boma, Hash40::new("special_s_throw"), 0.0, 1.0, false, 0.0, false, false,
        );
    } else {
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
        MotionModule::change_motion(
            boma, Hash40::new("special_air_s_throw"), 0.0, 1.0, false, 0.0, false, false,
        );
    }

    whip_kinetics_throw(fighter);

    fighter.sub_shift_status_main(L2CValue::Ptr(special_s_throw_main_loop as *const () as _))
}

/// STILL BEING TRANSCRIBED -- the rope flight. The source is gaogaen's SpecialSThrow main_loop,
/// already read once in the previous implementation's notes: CHECK_WALL_MOVE moves the captor
/// toward FLOAT_GAOGAEN_TARGET_X, FLAG_THROW_ROPE turns into FLAG_THROW_ROPE_END and sends
/// `hash40("gaogaen_swing_throw")` to LINK_NO_CAPTURE and a second event to the rope, and the
/// status ends when the motion does and the rope article exists.
unsafe extern "C" fn special_s_throw_main_loop(_fighter: &mut L2CFighterCommon) -> L2CValue {
    0.into()
}

/// NOT CALLED YET -- see the module header. Registration waits until all six statuses are
/// transcribed, so that the move is never half-owned by two implementations.
#[allow(dead_code)]
pub fn install() {
    Agent::new("pikachu")
        .status(Pre, *FIGHTER_STATUS_KIND_SPECIAL_S, special_s_pre)
        .status(Main, *FIGHTER_STATUS_KIND_SPECIAL_S, special_s_main)
        .status(Pre, RAT_STATUS_KIND_SPECIAL_S_THROW, special_s_throw_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_THROW, special_s_throw_main)
        .install();
}
