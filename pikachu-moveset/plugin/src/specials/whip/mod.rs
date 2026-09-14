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
// The catch, which is the one thing Incineroar's own code does not do. See catch.rs.
mod catch;

// ==== WHAT OF `gaogaen/param_special_s` THIS MODULE DOES NOT USE, AND WHY ====
//
// `tools/check-srcparams.sh` requires every param in the source object to be either read or
// declined here with a reason. These are the declines, grouped by why.
//
// READ BY THE VICTIM'S STATUS, NOT BY OURS. `docs/the-recording.md` catches these arriving on the
// victim: `mario1 ControlModule::start_clatter(85, 0, 5, ...)`, where 85/0/5 are exactly these
// three off the CAPTOR's param object. Our side never reads them; `src/params` answers them when
// the victim's status asks, which is the whole point of that module.
//   NOT MODELLED: special_s_catch_clatter_damage_mul_frame_add
//   NOT MODELLED: special_s_catch_clatter_dec_frame
//
// READ BY THE SOURCE'S PROLOGUE AND USED BY NO BRANCH OF IT. `0x710001d4b0` hoists twelve param
// reads before its dispatch -- generated code evaluates all its locals up front -- and these two
// are then never referenced by any of the six branches. Reading a param has no side effect, so
// omitting them changes nothing.
//   NOT MODELLED: special_air_s_catch_accel_y
//   NOT MODELLED: special_air_s_catch_speed_y_max
//
// THE CAPSULE DIMENSIONS BEYOND x1. `whip_update_areas` reduces each area to the 1-D interval its
// x offsets describe, because the victim is carried at our own height for the whole run, so the
// y span and the radius cannot change the answer. `node` names the bone the capsule hangs off --
// `top` for all three, which is the corroboration that the offsets are CAPTOR-RELATIVE and is
// why that function anchors them where it does.
//   NOT MODELLED: special_s_lariat_node special_s_lariat_size
//   NOT MODELLED: special_s_lariat_offset_y1 special_s_lariat_offset_x2 special_s_lariat_offset_y2
//   NOT MODELLED: special_s_lariat_end_node special_s_lariat_end_size
//   NOT MODELLED: special_s_lariat_end_offset_y1 special_s_lariat_end_offset_x2
//   NOT MODELLED: special_s_lariat_end_offset_y2
//   NOT MODELLED: special_s_failure_node special_s_failure_size
//   NOT MODELLED: special_s_failure_offset_y1 special_s_failure_offset_x2
//   NOT MODELLED: special_s_failure_offset_y2
//
// THE ROPE FLIGHT, AND THIS GROUP IS A LEAD RATHER THAN A DEAD END. These govern how the victim
// travels to the ropes and back, and the names are worth reading closely:
//
//     special_s_through_target_speed              11.2
//     special_s_through_target_return_speed        2.8
//     special_s_through_target_cancel_frame        30
//     special_s_through_target_speed_no_rope       1.6
//     special_s_through_target_cancel_frame_no_rope 30
//     special_s_through_target_speed_over_rope     1.6
//     special_s_through_target_motion_frame_fix    8
//     special_s_through_target_return_motion_frame_fix 8
//
// THE GAME HAS A DEFINED NO-ROPE CASE, which is exactly the case ADR 0004 puts this port in
// permanently. `_no_rope` and `_over_rope` mean Incineroar's own move already has to cope with
// there being no ropes to throw into, and the numbers for it are right here. Nothing reads them in
// the gaogaen NRO, so the code that applies them is the native rope/throw handler -- but the
// VALUES are the game's answer to the port's own situation, and driving the throw from them is the
// obvious next step for this move. Declined now because how they are applied is not yet known, and
// a speed applied the wrong way is worse than a speed not applied.
//   NOT MODELLED: special_s_through_target_speed special_s_through_target_return_speed
//   NOT MODELLED: special_s_through_target_cancel_frame special_s_through_target_speed_no_rope
//   NOT MODELLED: special_s_through_target_cancel_frame_no_rope
//   NOT MODELLED: special_s_through_target_speed_over_rope
//   NOT MODELLED: special_s_through_target_motion_frame_fix
//   NOT MODELLED: special_s_through_target_return_motion_frame_fix
//
// THE ONCE-PER-AIRTIME LOCKOUT. `state::invalid_special_air_s` is the flag this frame count feeds,
// and nothing in the NRO reads the param -- the engine enforces it. `src/specials/lockout.rs`
// already owns the port's version of this rule.
//   NOT MODELLED: special_s_invalid_capture_frame
//
// FINAL SMASH CAMERA. Present in every `param_special_*` object in the game and nothing to do with
// this move.
//   NOT MODELLED: critical_frame critical_slow_rate critical_zoom_rate critical_zoom_in_frame
//   NOT MODELLED: critical_camera_offset_x critical_camera_offset_y critical_1_on_1

use super::status::*;
use {
    smash::{
        lua2cpp::*,
        phx::*,
        app::{lua_bind::*, sv_kinetic_energy, KineticUtility, *},
        lib::{lua_const::*, L2CValue},
        hash40,
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
    // A whip status is starting, so the previous transition's two end lines are both behind us
    // and the next one may run its body again. See `end_line_first`.
    let entry = entry_of(boma);
    if entry < 8 {
        END_BODY_DONE[entry] = false;
    }
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
/// `reset_energy` pushed the way the source pushes it: SEVEN arguments, always.
///
/// `sv_kinetic_energy::*` take nothing but `lua_state_agent`; the arguments go through the lua
/// stack, which is why a function hook sees none of them and the recording never contained a
/// single one of these calls.
unsafe fn reset_energy(fighter: &mut L2CFighterCommon, id: i32, kind: i32, third: f32) {
    fighter.agent.clear_lua_stack();
    fighter.agent.push_lua_stack(&mut L2CValue::new_int(id as u64));
    fighter.agent.push_lua_stack(&mut L2CValue::new_int(kind as u64));
    // Zero in every call but one: the SPECIAL_S air branch passes `get_sum_speed_x` here.
    fighter.agent.push_lua_stack(&mut L2CValue::new_num(third));
    for _ in 0..4 {
        fighter.agent.push_lua_stack(&mut L2CValue::new_num(0.0));
    }
    sv_kinetic_energy::reset_energy(fighter.lua_state_agent);
}

/// `set_speed(id, v, 0)` -- three arguments, the third always zero.
unsafe fn set_speed(fighter: &mut L2CFighterCommon, id: i32, v: f32) {
    fighter.agent.clear_lua_stack();
    fighter.agent.push_lua_stack(&mut L2CValue::new_int(id as u64));
    fighter.agent.push_lua_stack(&mut L2CValue::new_num(v));
    fighter.agent.push_lua_stack(&mut L2CValue::new_num(0.0));
    sv_kinetic_energy::set_speed(fighter.lua_state_agent);
}

/// `set_brake(id, v, 0)` -- three arguments, the third always zero.
unsafe fn set_brake(fighter: &mut L2CFighterCommon, id: i32, v: f32) {
    fighter.agent.clear_lua_stack();
    fighter.agent.push_lua_stack(&mut L2CValue::new_int(id as u64));
    fighter.agent.push_lua_stack(&mut L2CValue::new_num(v));
    fighter.agent.push_lua_stack(&mut L2CValue::new_num(0.0));
    sv_kinetic_energy::set_brake(fighter.lua_state_agent);
}

/// `set_speed_mul(id, v)` -- two arguments.
unsafe fn set_speed_mul(fighter: &mut L2CFighterCommon, id: i32, v: f32) {
    fighter.agent.clear_lua_stack();
    fighter.agent.push_lua_stack(&mut L2CValue::new_int(id as u64));
    fighter.agent.push_lua_stack(&mut L2CValue::new_num(v));
    sv_kinetic_energy::set_speed_mul(fighter.lua_state_agent);
}

/// THE WHOLE KINETIC BEHAVIOUR OF ALL SIX WHIP STATUSES, WHICH IS ONE FUNCTION AND NOT THREE.
///
/// This replaces `whip_kinetics_enter`, `_throw` and `_followup`, which were reconstructed from
/// `docs/the-recording.md`. They are gone because the game does not have three functions. It has
/// one, at gaogaen `0x710001d4b0`, and every whip status main calls it:
///
///     SPECIAL_S          main  0x71000042f4  ->  0x710001d4b0
///     SPECIAL_S_THROW    main  0x7100004ad8  ->  0x710001d4b0
///     SPECIAL_S_SHOULDER main  0x7100005750  ->  0x710001d4b0
///     SPECIAL_S_LARIAT   main  0x7100006080  ->  0x710001d4b0
///     SPECIAL_S_FAILURE  main  0x71000068f0  ->  0x710001d4b0
///     SPECIAL_S_END      main  0x7100007150  ->  0x710001d4b0
///
/// It reads the current status out of the agent's global table and branches on it, so the faithful
/// port is ONE transcription with the same branch -- not three summaries of what a trace happened
/// to show. The difference is not cosmetic. The old `_enter` was the `!MOVE` branch with its air
/// `reset_energy(STOP, .., sum_speed_x, ..)` missing AND the `MOVE` branch's extra `clear(GRAVITY)`
/// and `clear(STOP)` mixed in: two mutually exclusive paths merged into one. That is precisely the
/// failure mode ADR 0006 exists to stop, and it is why the port's kinetics never matched.
///
/// WHY THE RECORDING COULD NOT HAVE GOT THIS RIGHT, and this is the general lesson: every
/// `sv_kinetic_energy` call here is a lua-stack call, so it carries no arguments a hook can read
/// and `enginetrace` never logged one. The recording remains ground truth for WHICH branch runs.
/// It was never capable of being ground truth for what is INSIDE a branch.
///
/// `flag` is the source's second parameter. ALL SIX CALL SITES PASS TRUE -- each is a literal
/// `mov w1, #0x1` into an `L2CValue` ctor immediately before the `bl` -- so the `!flag`
/// sub-branches are unreachable in practice. Kept as a parameter because that is the source's
/// shape and because `whip_move_flags` passes it too.
///
/// NOT EVERY PARAM THE SOURCE READS IS READ HERE. The prologue of `0x710001d4b0` fetches twelve
/// values up front -- generated code hoists all its locals -- but `special_air_s_catch_accel_y`,
/// `special_air_s_catch_speed_y_max` and the three `special_s_catch_clatter_*` are never used by
/// any branch. Reading a param has no side effect, so omitting them changes nothing; the clatter
/// three are read by the VICTIM's status, not by this one (see `docs/the-recording.md`, where
/// `start_clatter(85, 0, 5, ..)` arrives on `mario1`).
unsafe fn whip_kinetics(fighter: &mut L2CFighterCommon, flag: bool) {
    let boma = fighter.module_accessor;
    // The source reads `agent->global_table[9]`; this is the engine's own cache of that value.
    let status = StatusModule::status_kind(boma);
    let ground = fighter.global_table[0x16] == *SITUATION_KIND_GROUND;

    let motion = *FIGHTER_KINETIC_ENERGY_ID_MOTION;
    let gravity = *FIGHTER_KINETIC_ENERGY_ID_GRAVITY;
    let control = *FIGHTER_KINETIC_ENERGY_ID_CONTROL;
    let stop = *FIGHTER_KINETIC_ENERGY_ID_STOP;

    if status == *FIGHTER_STATUS_KIND_SPECIAL_S {
        // The catch itself. Two paths, chosen by the MOVE flag that `whip_move_flags` toggles:
        // before the slide starts (and after it ends) the body is held still; during the slide the
        // motion energy drives it at `special_s_start_speed_mul`.
        if !WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_MOVE) {
            if !flag {
                if ground {
                    KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_GROUND_STOP);
                } else {
                    KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_FALL);
                }
            } else if ground {
                KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_GROUND_STOP);
            } else {
                KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_FALL);
                // THE CALL THE RECORDING COULD NOT SEE, and the one that carries a real value:
                // the third argument is the airborne carry-over speed.
                let sum_x =
                    KineticModule::get_sum_speed_x(boma, *KINETIC_ENERGY_RESERVE_ATTRIBUTE_MAIN);
                reset_energy(fighter, stop, *ENERGY_STOP_RESET_TYPE_AIR, sum_x);
                KineticModule::enable_energy(boma, stop);
            }
            KineticUtility::clear_unable_energy(motion, boma);
            KineticUtility::clear_unable_energy(control, boma);
        } else {
            if !flag {
                return;
            }
            if ground {
                reset_energy(fighter, motion, *ENERGY_MOTION_RESET_TYPE_GROUND_TRANS, 0.0);
                set_speed_mul(fighter, motion, WorkModule::get_param_float(boma, hash40("param_special_s"), hash40("special_s_start_speed_mul")));
            } else {
                reset_energy(fighter, motion, *ENERGY_MOTION_RESET_TYPE_AIR_TRANS, 0.0);
                set_speed_mul(fighter, motion, WorkModule::get_param_float(boma, hash40("param_special_s"), hash40("special_air_s_start_speed_mul")));
            }
            KineticModule::enable_energy(boma, motion);
            KineticUtility::clear_unable_energy(gravity, boma);
            KineticUtility::clear_unable_energy(control, boma);
            KineticUtility::clear_unable_energy(stop, boma);
        }
    } else if status == RAT_STATUS_KIND_SPECIAL_S_THROW {
        // The throw keeps whatever the catch left moving and brakes it -- no `change_kinetic` at
        // all, which is why the throw's `init_settings` passes GROUND_CORRECT_KIND_KEEP.
        KineticUtility::clear_unable_energy(motion, boma);
        KineticUtility::clear_unable_energy(gravity, boma);
        KineticUtility::clear_unable_energy(control, boma);
        if !flag {
            return;
        }
        let sum_x = KineticModule::get_sum_speed_x(boma, *KINETIC_ENERGY_RESERVE_ATTRIBUTE_MAIN);
        if ground {
            reset_energy(fighter, stop, *ENERGY_STOP_RESET_TYPE_GROUND, 0.0);
            set_speed(fighter, stop, sum_x * WorkModule::get_param_float(boma, hash40("param_special_s"), hash40("special_s_catch_speed_x_mul")));
            set_brake(fighter, stop, WorkModule::get_param_float(boma, hash40("param_special_s"), hash40("special_s_catch_brake_x")));
        } else {
            reset_energy(fighter, stop, *ENERGY_STOP_RESET_TYPE_AIR, 0.0);
            set_speed(fighter, stop, sum_x * WorkModule::get_param_float(boma, hash40("param_special_s"), hash40("special_air_s_catch_speed_x_mul")));
            set_brake(fighter, stop, WorkModule::get_param_float(boma, hash40("param_special_s"), hash40("special_air_s_catch_brake_x")));
        }
        KineticModule::enable_energy(boma, stop);
    } else if status == RAT_STATUS_KIND_SPECIAL_S_SHOULDER
        || status == RAT_STATUS_KIND_SPECIAL_S_LARIAT
        || status == RAT_STATUS_KIND_SPECIAL_S_FAILURE
    {
        // THE THREE FOLLOW-UPS ARE BYTE-FOR-BYTE THE SAME BRANCH in the source -- three separate
        // `else` blocks with identical bodies -- except that FAILURE gates the reset on `flag`
        // while SHOULDER and LARIAT do not. Since every caller passes true, the gate never differs;
        // it is written out anyway so the diff against the source is checkable.
        let gated = status != RAT_STATUS_KIND_SPECIAL_S_FAILURE || flag;
        if gated {
            if ground {
                reset_energy(fighter, motion, *ENERGY_MOTION_RESET_TYPE_GROUND_TRANS, 0.0);
            } else {
                reset_energy(fighter, motion, *ENERGY_MOTION_RESET_TYPE_AIR_TRANS, 0.0);
            }
            KineticModule::enable_energy(boma, motion);
        }
        KineticUtility::clear_unable_energy(gravity, boma);
        KineticUtility::clear_unable_energy(control, boma);
        KineticUtility::clear_unable_energy(stop, boma);
    } else if status == RAT_STATUS_KIND_SPECIAL_S_END {
        // Four clears and nothing else: hand every energy back to the common movement code.
        KineticUtility::clear_unable_energy(motion, boma);
        KineticUtility::clear_unable_energy(gravity, boma);
        KineticUtility::clear_unable_energy(control, boma);
        KineticUtility::clear_unable_energy(stop, boma);
    }
    // Any other status falls through to the source's `LAB_710001ef68`, which is the bare return.
}

/// `FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_MOVE_START` / `_MOVE_END` -> `_MOVE`, gaogaen
/// `0x7100023730`, called from SPECIAL_S main when `StopModule::is_stop` is true.
///
/// The ACMD script raises MOVE_START and MOVE_END at the frames the slide should begin and end;
/// this converts either edge into the steady MOVE flag and RE-RUNS THE KINETICS IMMEDIATELY, which
/// is how the whip changes from held-still to sliding mid-status without a status change. Both
/// re-runs pass `true`.
///
/// These three are STATUS work, not instance work, so they go through `WorkModule` directly -- see
/// the note in `state.rs` about which of gaogaen's work ids are safe to reuse and why.
unsafe fn whip_move_flags(fighter: &mut L2CFighterCommon, skip: bool) {
    if skip {
        return;
    }
    let boma = fighter.module_accessor;
    if WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_MOVE_START) {
        WorkModule::off_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_MOVE_START);
        WorkModule::on_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_MOVE);
        whip_kinetics(fighter, true);
    }
    if WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_MOVE_END) {
        WorkModule::off_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_MOVE_END);
        WorkModule::off_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_MOVE);
        whip_kinetics(fighter, true);
    }
}

/// RELEASING THE VICTIM, gaogaen `0x71000206c0`, called from SHOULDER, LARIAT and FAILURE
/// `status_end`, and inlined verbatim at the end of SPECIAL_S_THROW.
///
/// THIS IS THE FUNCTION THE PORT HAS BEEN GUESSING AT FOR SEVEN ROUNDS. `docs/the-recording.md`
/// records that the port's first two attempts -- `CatchModule::set_catch(boma, victim_id)` to claim
/// and `CaptureModule::capture_cut(..)` to release -- are both calls the real fighters never make,
/// and that Donkey Kong ends a carry with `CatchModule::catch_cut`. Incineroar does something
/// different again, and this is it: he does not cut the catch at all. He tears down the LINK.
///
/// The events are two hash40s that resolve to nothing. `tools/hash40.sh` reports them as 24 and 20
/// characters with every source searched, which means the L2C compiler baked them from strings that
/// are in neither the executable nor the archive. They are passed as literals, the same way
/// `docs/the-recording.md` carries `0x2a5eaa0cb9` unresolved. A hash is all the engine wants.
/// RETURNS WHETHER IT ACTUALLY RELEASED. The source returns 0 from the path that tore the link
/// down and 1 from both early bail-outs, and `whip_followup_loop` reads that value -- it clears the
/// stored victim id only when the teardown really happened. Inverted to a plain `bool` here, since
/// `true` meaning "released" reads the way the call site uses it.
unsafe fn whip_release(fighter: &mut L2CFighterCommon) -> bool {
    // THE GUARD IS THE NEXT STATUS, AND THE ONE THAT WAS HERE INSTEAD IS THE WHOLE REGRAB BUG.
    //
    // `if CaptureModule::is_capture(boma) { return false }` stood at the top of this function and
    // is nowhere in the source. It was justified as "a Pikachu who got grabbed mid-whip must not
    // tear down the grab holding him", which is a real concern and the wrong predicate:
    //
    //     CATCH p0 starting: mine is_catch=false ignore_catching=false is_capture=TRUE
    //       | 0x10000 st=WAIT(0x0) is_capture=true ignore_catching=false captured_by=0x0
    //
    // `is_capture` is TRUE ON THE CAPTOR TOO. Ten of the twelve grab attempts in pt23 read it
    // that way on Pikachu himself. So the guard fired on every single whip -- precisely when the
    // release was needed -- and returned before `drop_capture` could clear the engine's record
    // off the victim. Mario is sitting in WAIT in that line with `captured_by=0x0`, which is
    // Pikachu, and a fighter the engine believes is captured cannot be caught by anybody.
    //
    // That is "grab after whip and cargo throw is still broken", and it survived the previous
    // round because the previous round blamed the `End` line. `End` fires fine -- 129 times in
    // pt23, and the up-B now costs its double jump, which is the same mechanism proving itself.
    //
    // WHAT THE SOURCE ACTUALLY GUARDS ON, `SpecialSThrow_end` `0x7100004ba0`, first statement:
    //
    //     next = globalTable[0xb]
    //     if next != SPECIAL_S_SHOULDER && next != SPECIAL_S_LARIAT
    //     && next != SPECIAL_S_FAILURE  && next != SPECIAL_S_END { <the whole teardown> }
    //
    // The same shape as DK's shoulder `exit.c`: do not let go of the victim when the status you
    // are leaving for is another part of the same move. Nothing about being captured.
    let next = &fighter.global_table[0xb];
    if *next == RAT_STATUS_KIND_SPECIAL_S_SHOULDER
        || *next == RAT_STATUS_KIND_SPECIAL_S_LARIAT
        || *next == RAT_STATUS_KIND_SPECIAL_S_FAILURE
        || *next == RAT_STATUS_KIND_SPECIAL_S_END
    {
        return false;
    }
    whip_release_teardown(fighter);
    true
}

/// The body of the release, with the next-status guard already decided.
///
/// Split out because `whip_followup_loop`'s watchdog calls the release from inside a main loop,
/// where `globalTable[0xb]` holds whatever the last transition left there and answering the
/// guard with it would be reading a stale value as if it were a decision.
unsafe fn whip_release_teardown(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let link = *LINK_NO_CAPTURE;

    // The rope article is retired here in the source, under `if !is_flag(THROW_ROPE_END)`.
    // Omitted under ADR 0004: there is no rope to retire.

    if LinkModule::is_link(boma, link) {
        // `link` splices the slot into both the parent and the child list, so the fighter being
        // thrown is reachable as this object's PARENT here and as its NODE below. Both directions
        // are torn down, in this order.
        //
        // `get_parent_id(.., TRUE)` -- the source passes true and this passed false.
        let parent = LinkModule::get_parent_id(boma, link, true);
        let thrown = state::swing_thrown_fighter(entry_of(boma));
        if parent as u32 == thrown {
            // `!is_flag(THROW_ROPE_END)`, and the event hash is the same one the node event uses.
            // This read `is_flag(FLAG_HIT)` -- the wrong flag, un-negated -- and sent
            // `0x18f836a168`, a hash that appears nowhere in the decomp, nowhere in
            // `docs/the-recording.md` and nowhere else in this repository. Both come straight out
            // of `SpecialSThrow_end` now.
            if !WorkModule::is_flag(
                boma,
                *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_THROW_ROPE_END,
            ) {
                LinkModule::send_event_parents(boma, link, Hash40 { hash: 0x1465ed7a5c });
            }
            LinkModule::unlink(boma, link);
        }
    }
    // A SIBLING OF THE BLOCK ABOVE, NOT NESTED INSIDE IT. The port had it nested, so the node
    // direction was only ever torn down when the parent direction still existed -- and a victim
    // who had already unlinked themselves left our side of the link standing.
    if LinkModule::is_linked(boma, link) {
        LinkModule::send_event_nodes(boma, link, Hash40 { hash: 0x1465ed7a5c }, 0);
        if LinkModule::is_link(boma, link) {
            LinkModule::unlink(boma, link);
        }
    }
    // ---- BELOW HERE IS NOT PART OF THE TRANSCRIPTION ----
    //
    // A RELEASE SAFETY NET, AND IT EXISTS BECAUSE ADR 0004 REMOVES THE NORMAL WAY OUT.
    //
    // For Incineroar the victim leaves on their own: CATCHED -> THROWN when the throw event
    // arrives, and THROWN ends by flinging them away. Their `CATCHED` main loop
    // (common 0x710028bb60) is three lines and the ONLY escape it offers is
    // `change_status(FIGHTER_STATUS_KIND_CAPTURE_CUT)` when their clatter runs out.
    //
    // So a victim who is still in a SWING_GAOGAEN_* status when the whip ends, with the link torn
    // down above, has no parent and a clatter that may still be running -- and their model
    // constraint is still bound to our "throw" bone, because `set_model_constraint_pos_ort` is
    // undone by `CAPTURE_CUT` and by nothing else. That is exactly the failure this project has
    // already seen once and recorded: "the victim's model stayed constrained to the captor's
    // `throw` bone -- Pikachu and Mario welded together".
    //
    // `CAPTURE_CUT` is not an invention: it is the status the victim's own loop escapes to, and
    // the engine's standard end-of-capture. Forcing it only when they are still in a whip status
    // means a victim who already left by the native path is untouched.
    let entry = entry_of(boma);
    let id = state::swing_thrown_fighter(entry);
    if let Some(vboma) = crate::victim::boma(id) {
        let st = StatusModule::status_kind(vboma);
        if st == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED
            || st == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_THROWN
            || st == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_LARIAT
            || st == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_SHOULDER
            || st == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_FAILURE
        {
            crate::telemetry::sink::diag(format!(
                "[rat] WHIP p{entry} victim {id:#x} still in {st:#x} after release -- forcing \
                 CAPTURE_CUT so the model constraint is undone"
            ));
            crate::victim::request_status(vboma, *FIGHTER_STATUS_KIND_CAPTURE_CUT, "whip release");
        }
    }

    // AND THE CAPTURE THIS MODULE MADE BY HAND IS DROPPED BY HAND, on every exit.
    //
    // `whip::catch::take_captive` calls `CaptureModule::capture` and links BOTH ends, and this
    // function unlinks the two link numbers the source uses -- but the engine's own record of the
    // catch, and `ignore_catching`, and any pending `catch_cut` request, were nobody's job. The
    // playtest: "not after a lariat still". `drop_capture` is the same call the carry's exits make,
    // for the same reason: Incineroar reaches his follow-ups out of a catch the engine established,
    // so his teardown is the engine's. Ours is not.
    // `true`: `whip::catch::take_captive` calls `CaptureModule::capture` by hand, so this port
    // owns the record and has to undo it.
    crate::victim::drop_capture(entry, "whip over", true);
    // Stop tracing and forget the victim, so a stale id cannot make the next whip's
    // `whip_victim_held` watchdog answer about the previous one's victim.
    crate::victim::untrace(entry);
    crate::victim::forget(entry, "whip over");
    state::set_swing_thrown_fighter(entry, *BATTLE_OBJECT_ID_INVALID as u32);
}

/// THE `End` LINE NEVER REACHES THIS PORT'S OWN STATUSES, AND THAT IS THE WHOLE REGRAB LOCKOUT.
///
/// Every one of the six whip statuses puts its teardown in `status_end`, because that is where
/// Incineroar puts his -- `vendor/ssbu-status/.../gaogaen/status/SpecialSThrow/` holds `end.c`
/// and no `exit.c`, and under ADR 0006 the line the source has is the line this port registers.
/// `whip_release` is the body of it: the two unlinks, the CAPTURE_CUT safety net, and
/// `victim::drop_capture`, which is what clears the engine's record of the catch off the victim.
///
/// IT HAS NEVER RUN. Not once, across every recorded playtest:
///
///     whip catches logged, pt19 + pt21 + pt22            ~100
///     `dropped the capture (whip over)`                     0
///     `dropped the capture (throw over|carry released)`     15   <- the carry, which uses `Exit`
///
/// `drop_capture`'s closing diag is unconditional, so zero lines means zero calls. The two
/// `TRACE pika0 st=0x1f5 LinkModule::unlink(link=0)` lines in pt22 are not evidence against it:
/// they are `end_main`'s own unlink at the bottom of this file, running in 0x1f5 because that is
/// the status whose Main they are.
///
/// So the victim keeps `is_capture` forever, and a fighter the engine believes is captured cannot
/// be caught. That is the reported bug exactly -- and the CATCH digests say the same thing from
/// the other side, because a BLOCKED GRAB IS NOT A WHIFF:
///
///     CATCH 7f  -> CATCH_PULL   a catch that CONNECTED (Pikachu's catch frames are 6-7)
///     CATCH 51f -> WAIT         a real whiff: the whole fifty-frame animation
///     CATCH 8f  -> WAIT         connected, then died instead of converting   <- the lockout
///
/// WHY `Exit` AND NOT SOMETHING ELSE. Both lines are dispatched from the same engine function,
/// one after the other: smashline hooks the End line at `+0x689c` and the Exit line at `+0x6950`,
/// and `vendor/main/common-nro-symbols.txt` puts both offsets inside
/// `L2CFighterBase::sub_end_status_msc` (0x7100006730 -> 0x71000069f0). `Exit` therefore runs at
/// the same moment of the same transition, and the carry proves it reaches a custom status. This
/// is not a different behaviour; it is the source's `end` body on the only line that arrives.
///
/// `End` STAYS REGISTERED. It costs nothing, it is the line the source has, and registering both
/// is what makes the next log state which one the engine actually called.
static mut END_BODY_DONE: [bool; 8] = [false; 8];

/// Is this the first of the transition's two end lines to arrive?
///
/// Latched, because `End` and `Exit` both fire from `sub_end_status_msc` and `failure_end`'s
/// `reverse_lr` is NOT idempotent -- running it twice would turn Pikachu back around and look
/// exactly like nothing had happened.
///
/// Cleared in `whip_pre`, which is safe because the next status's Pre always runs after both end
/// lines of the status it replaced.
unsafe fn end_line_first(fighter: &mut L2CFighterCommon, line: &str) -> bool {
    let entry = entry_of(fighter.module_accessor);
    if entry >= 8 {
        return true;
    }
    if END_BODY_DONE[entry] {
        return false;
    }
    END_BODY_DONE[entry] = true;
    crate::telemetry::sink::diag(format!(
        "[rat] WHIP p{entry} status_end body reached via the {line} line, leaving {:#x}",
        StatusModule::status_kind(fighter.module_accessor)
    ));
    true
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
    // THE ROPE HAS TO EXIST BEFORE THE CATCH, NOT BEFORE THE THROW.
    //
    // `whip_update_areas` sets this too, but it only runs from the THROW status's loop -- and the
    // victim has already read `is_flag(GENERATED_ROPE)` by then. `status_SwingGaogaenCatched`
    // stores its throw speed off that flag while the captor is still in SPECIAL_S, which is why
    // the first attempt at the rope branch changed nothing measurable: the flag was still false at
    // the only moment it was asked. The flight was identical to the no-rope one, frame for frame:
    // +14.74 at f30, -10.29 at f40, -18.29 at f50.
    update_rope_x(fighter);

    // ONE LINE PER WHIP, so a playtest that reports nothing can be told apart from a playtest
    // where the status never ran. The first test of this rewrite produced no WHIP lines at all and
    // that was indistinguishable from the module not being installed.
    crate::telemetry::sink::diag(format!(
        "[rat] WHIP p{entry} SPECIAL_S main: ground={} move_start={}",
        fighter.global_table[0x16] == *SITUATION_KIND_GROUND,
        WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_MOVE_START)
    ));

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

    whip_kinetics(fighter, true);

    // SOURCE: `StopModule::is_stop` -> `0x7100023730`. `is_stop` is true while hitlag holds the
    // fighter, which is when the slide's start and end edges have to be consumed -- otherwise the
    // MOVE flag would be read a frame late.
    if StopModule::is_stop(boma) {
        whip_move_flags(fighter, false);
    }

    // DELIBERATELY NOT TRANSCRIBED: `GroundModule::select_cliff_hangdata` at 0x71000043c4.
    //
    // Its argument is `lua_const[0xb610]`, which is not one of the 19,310 offsets
    // `vendor/ssbu-status/lua_dict.txt` names -- but the dict makes the shape unmistakable, because
    // every constant of this family is fighter-scoped:
    //
    //     3380:FIGHTER_LINK_CLIFF_HANG_DATA_AIR_LASSO
    //     5598:FIGHTER_FOX_CLIFF_HANG_DATA_SPECIAL_S
    //
    // So 0xb610 is `FIGHTER_GAOGAEN_CLIFF_HANG_DATA_SPECIAL_S`: an index into GAOGAEN's own
    // cliff-hang data table. It is exactly the class of value ADR 0006's rule forbids reusing --
    // Pikachu's table has no whip entry, and passing gaogaen's index would select whatever happens
    // to sit at that index for him. Omitted rather than guessed; the cost is that the whip does not
    // get its own ledge-grab box while it runs, which is a bounded, stated gap.
    fighter.sub_shift_status_main(L2CValue::Ptr(special_s_main_loop as *const () as _))
}

/// SPECIAL_S's own per-frame loop, gaogaen `0x71000239d0`.
///
/// I OVERWROTE A CORRECT COMMENT HERE WITH A WRONG ONE. The note that used to be at this spot read
/// `0x7100023970 -> FUN_7100023730` and I replaced it with a claim that this was a misreading of
/// the loop address. It was not. BOTH functions are real and both matter:
///
///   `0x71000239d0`  the main_loop, handed to `sub_shift_status_main` by the adrp+add at
///                   `0x71000043d0` -- the generic skeleton, same as the four follow-ups
///   `0x7100023970`  a wrapper that calls the MOVE-flag toggler, INSTALLED AS A CALLBACK by
///                   `table[0x15] = <ptr>` at `0x7100004380`-`0x7100004384`
///
/// So the old note had hold of the callback and I had hold of the loop, and I called its address a
/// mistake instead of checking. `--dataxref 0x7100023970` prints the assignment site in one line
/// and would have settled it before I wrote anything.
///
/// THE CALLBACK IS WHY THE FIRST PLAYTEST OF THIS REWRITE DID NOT MOVE AT ALL. `whip_move_flags`
/// is the only thing that turns the ACMD's frame-1 `FLAG_MOVE_START` into the steady `FLAG_MOVE`,
/// and `whip_kinetics` reads `FLAG_MOVE` to decide between holding still and sliding at
/// `special_s_start_speed_mul`. I transcribed the toggler but only from `main`'s one-shot
/// `StopModule::is_stop` branch, so on a whip with no hitlag it never ran, `FLAG_MOVE` was never
/// set, and every frame took the stand-still branch.
///
/// CALLED FROM HERE INSTEAD OF FROM A CALLBACK SLOT, and the equivalence is arguable rather than
/// assumed: `whip_move_flags` acts only when MOVE_START or MOVE_END is set and CLEARS the flag it
/// acted on, so it is edge-triggered and idempotent. Running it once per frame from the loop is
/// therefore indistinguishable from the engine running it once per frame from `table[0x15]`, up to
/// ordering within the frame. Reproducing the callback registration would mean reverse-engineering
/// what slot 0x15 is and when L2C fires it, which is a deeper hole than the behaviour justifies --
/// and unlike a guess, this one is checkable: if the slide starts on the right frame, the ordering
/// did not matter.
unsafe extern "C" fn special_s_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    // The per-frame edge conversion the source installs as a callback. Must run before
    // `whip_followup_loop`, which is where `whip_kinetics` reads FLAG_MOVE.
    whip_move_flags(fighter, false);
    // THE ONE NON-TRANSCRIBED LINE IN THE WHOLE MOVE, and it is here rather than inside
    // `whip_followup_loop` so that the transcription stays a transcription. Incineroar's status
    // scripts never detect the catch -- the engine does it in native code keyed on fighter kind,
    // which does not run for a Pikachu. See `catch.rs` for the evidence and the substitution.
    if catch::try_catch(fighter) {
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_THROW.into(), true.into());
        return 0.into();
    }
    whip_followup_loop(fighter)
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
/// `generate_rope` IS NOT CALLED, AND CALLING IT SPAWNED A THUNDER JOLT.
///
/// The note that used to be here said it WAS called "because whether it produces a rope for a
/// Pikachu is a measurement rather than an assumption". That reasoning admitted two outcomes -- a
/// rope, or nothing -- and the real one was a third: IT PRODUCES A DIFFERENT ARTICLE. An article id
/// is a per-fighter enumeration, so Incineroar's rope index names whatever Pikachu keeps at that
/// index, and `deforeign`'s own header spells out what lives at the bottom of his list:
///
///     FIGHTER_ELIGHT_GENERATE_ARTICLE_ESWORD  = 0x0  = Pikachu's DENGEKIDAMA
///
/// DENGEKIDAMA is the Thunder Jolt ball. The playtest reported the victim taking "7.5% of electric
/// damage" that "lines up exactly with thunder jolt's damage and visuals, and stales at the same
/// rate" -- because it WAS a Thunder Jolt, spawned by Incineroar's own rope generator running on a
/// Pikachu. The measurement the old note leaned on was also never actually written: there is no
/// `ArticleModule::is_exist` call before or after it anywhere.
///
/// THIS IS THE POINT OF ADR 0006's RULE, ARRIVING FROM A DIRECTION THE RULE DID NOT COVER. The call
/// is word-for-word correct -- it is Incineroar's own exported native function, by name, with his
/// own argument. A verbatim port of a call whose ARGUMENT SPACE is per-fighter is still wrong on a
/// different fighter, and `deforeign` exists precisely to strip that class out of generated ACMD --
/// but it never processes hand-written status code, so nothing caught it here.
///
/// Omitting it is not a behaviour change of mine; it is the consequence of ADR 0004, which already
/// decided this port ships no rope. Every CONSUMER of the rope is already omitted for that reason
/// (the two `ArticleModule::change_status_exist` calls and the rope's `send_event_nodes`). Keeping
/// the generator while removing everything that reads it was the inconsistency. The only way to
/// make this line faithful rather than absent is to ship the rope article, which is ADR 0004's
/// open question and not one to reopen silently.
unsafe extern "C" fn special_s_throw_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;

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

    whip_kinetics(fighter, true);

    fighter.sub_shift_status_main(L2CValue::Ptr(special_s_throw_main_loop as *const () as _))
}

/// THE ROPE FLIGHT AND THE FOLLOW-UP DECISION -- gaogaen `0x71000223b0`.
///
/// THIS FUNCTION IS THE ANSWER TO THE EARLY-PRESS BEHAVIOUR, which the port had been approximating
/// with frame windows for seven rounds. The real rule has nothing to do with frames:
///
///   * Attack or special pressed OUTSIDE the lariat area does NOT change status. It only raises
///     `FLAG_SHOULDER` and the whip carries on.
///   * When the whip later enters the lariat area, `FLAG_SHOULDER` already being up sends it to
///     SPECIAL_S_SHOULDER -- the body slam, "thrown into the air".
///   * A press made INSIDE the lariat area goes to SPECIAL_S_LARIAT instead -- the spin into the
///     ropes.
///   * `FLAG_FAILURE_AREA` sends it to SPECIAL_S_END.
///
/// So "early press" and "late press" are not two timings of one check; they are two different
/// checks, and which one fires depends on flags the ACMD script raises as the run progresses.
///
/// IT ALSO RECORDS WHERE. On entering the lariat area it stores the VICTIM's `pos_x` into
/// `LARIAT_AREA_HIT_POS_X`, and on the button press it stores it again into
/// `LARIAT_BUTTON_PUSH_POS_X`. Those are exactly the two floats `lariat_main`'s motion-rate formula
/// divides -- so the lariat's animation speed is set by how far along the run the button came, and
/// the two halves of that mechanism are in different functions.
///
/// THE FIRST THING IT DOES IS THE WATCHDOG. If the victim has stopped being a validly held victim,
/// the rope is retired and the status goes straight to SPECIAL_S_END, whatever else was pending.
/// THE AREA FLAGS, WHICH ARE THE ONE THING ADR 0004 COSTS THIS MOVE.
///
/// `FLAG_LARIAT_AREA`, `_LARIAT_END_AREA`, `_LARIAT_AREA_FIRST` and `_FAILURE_AREA` are read by the
/// THROW loop and written by NOBODY this port can reach. They are not in any gaogaen ACMD script
/// (the seven that are: DAMAGE_CUT, ENABLE_GRAVITY, MOVE_END, MOVE_START, THROW_ROPE,
/// LARIAT_HIT_FRAME, AIR_CONTROL), not in the gaogaen NRO, and not in a rope NRO -- there isn't
/// one. `prebuilt:/nro/release/lua2cpp_gaogaen_rope.nro` is Missing in data.arc, no fighter NRO
/// carries a `create_agent_weapon_*`, and main exports only three gaogaen symbols. The rope's code
/// is an unnamed static in main's `.text`, belonging to an article this port does not ship.
///
/// SO THE GEOMETRY IS TAKEN FROM THE PARAMS, WHICH ARE THE SAME NUMBERS THAT CODE USES. Three
/// capsules, out of `gaogaen/param_special_s` and answered for Pikachu by `crate::params`:
///
///     special_s_lariat       size 2  x1 -24  y1 12  x2 -24  y2 2
///     special_s_lariat_end   size 2  x1 -10  y1 12  x2 -10  y2 2
///     special_s_failure      size 2  x1  -6  y1 12  x2  -6  y2 2
///
/// Vertical segments at three captor-relative x offsets, all behind him -- which is where the whip
/// throws. Two independent corroborations that this reading is right:
///
///   1. A previous round derived LARIAT_X = 24.0, LARIAT_END_X = 10.0 and FAILURE_X = 6.0 by hand
///      and got the same three magnitudes.
///   2. `lariat_main`'s rate formula is `|button_push_x - area_hit_x| / area_length`, which is only
///      a 0..1 ratio if the area SPANS -24 to -10 -- and |-24 - -10| is 14, which is then exactly
///      what `area_length` has to be. The formula and the two params agree, and neither was chosen
///      to fit the other.
///
/// So the area is the interval the victim travels on the way back from the ropes, the press
/// position inside it sets the animation speed, and passing -6 is the whiff. What is inferred here
/// is the ANCHOR (captor-relative) and nothing else.
/// WHERE THE ROPES ARE, IN WORLD COORDINATES, SO THE VICTIM'S OWN CODE CAN FLY THEM THERE.
///
/// `off` in `whip_update_areas` is `(victim_x - my_x) * lr`, so the rope line -- `off ==
/// lariat_offset_x1` -- is at `my_x + lariat_offset_x1 * lr`. That is the same number the port was
/// already watching for as its rebound trigger; the change is that it is handed to the engine as
/// `FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLOAT_ROPE_X` (through `src/params`) instead of watched.
///
/// LATCHED THE MOMENT THE VICTIM LEAVES HIS HANDS, NOT REFRESHED FOR THE WHOLE MOVE.
///
/// It used to refresh every frame, justified as "a real rope article would move with him". That
/// was a guess and it is the reported "mario seems to travel a variable distance for the alolan
/// whip throw-out": the victim's own status reads this float to decide where to stop, and the
/// captor is still sliding on `special_s_catch_brake_x` while they fly -- so the finish line moved
/// under them by however much he happened to have left to slide. A rope is a thing in the world.
/// It is placed once.
///
/// Still refreshed while they are HELD, because until the throw fires the rope has not been placed
/// yet and the captor's lunge is what decides where it goes.
unsafe fn update_rope_x(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    if let Some(vboma) = crate::victim::boma(state::swing_thrown_fighter(entry)) {
        let vst = StatusModule::status_kind(vboma);
        let flying = vst == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_THROWN
            || vst == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_ATTACH_ROPE
            || vst == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_RETURN;
        if flying && state::rope_x_set(entry) {
            return;
        }
    }
    // `ROPE_OFFSET_X`, NOT `special_s_lariat_offset_x1`. The lariat offsets describe the WINDOW
    // the follow-up connects in, which is a different thing that happens to be on the same axis;
    // the docs already recorded that -24 is not the rope and this is the read that was still
    // using it.
    state::set_rope_x(
        entry,
        PostureModule::pos_x(boma) + ROPE_OFFSET_X * PostureModule::lr(boma),
    );
    state::set_rope_x_set(entry, true);
}

unsafe fn whip_update_areas(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    let Some(vx) = victim_pos_x(boma) else {
        return;
    };
    // The victim's offset in the captor's facing frame: positive is in front, so the whole of this
    // happens at negative values.
    let off = (vx - PostureModule::pos_x(boma)) * PostureModule::lr(boma);

    let lariat_x = WorkModule::get_param_float(boma, hash40("param_special_s"), hash40("special_s_lariat_offset_x1"));
    let lariat_end_x = WorkModule::get_param_float(boma, hash40("param_special_s"), hash40("special_s_lariat_end_offset_x1"));
    let failure_x = WorkModule::get_param_float(boma, hash40("param_special_s"), hash40("special_s_failure_offset_x1"));

    state::set_lariat_area_length(entry, (lariat_x - lariat_end_x).abs());

    update_rope_x(fighter);

    // MEASURED, BECAUSE THE REBOUND TRIGGER NEVER FIRED AND THERE ARE FOUR REASONS IT MIGHT NOT.
    //
    // `rope rebound` appeared zero times in the log, so `lariat_been_out` never went true, so the
    // victim was never sampled inside [-24, -10]. That is either: they do not travel at all (the
    // STOP energy from `..._THROWN_SPEED_X` is not moving them), they travel too little (brake and
    // limit eat the 1.6), they travel the WRONG WAY (an `lr` sign flip would make `off` positive,
    // and `whip_release`'s FAILURE path does call `reverse_lr`), or this function is not running
    // during the flight at all. One line every ten frames separates all four, and the victim's own
    // status frame is in it so the 30-frame `MOVE_CANCEL_FRAME` window is visible too.
    // THE ROPE IS AT THE FAR EDGE (-24), NOT AT THE NEAR ONE, and conflating the two slung the
    // victim back the instant he passed behind Pikachu.
    //
    // The playtest with the thresholds finally non-zero: "alolan whip now immediately slings mario
    // the opposite direction of the throw against the ropes". The flight in the log is
    // +16 -> -10.29 (f40) -> -18.29 (f50) -> -26.29 (f60) -> -42 (f70), so the window [-24, -10]
    // is first touched at its NEAR edge, -10, while he is still travelling OUT. The rebound fired
    // there, `SwingGaogaenReturn` reversed his facing, and he was thrown back before he had gone
    // anywhere.
    //
    // `special_s_lariat_offset_x1` = -24 is where the ropes are; -10 is
    // `special_s_lariat_end_offset_x1`, the line past which the window CLOSES on the way home.
    //
    // -24 IS NOT THE ROPE. That reading drove the rebound off this line for four rounds, and the
    // native measurement says the rope catches the victim at -41.55 -- seventeen units further --
    // while -24 is simply the first of three tripwires they cross going out and again coming back.
    // The rebound is now timed, not placed: see `whip_flight`. All this line does is latch "they
    // have been out past the lariat line at least once", which is what makes the interval
    // meaningful on the return trip and nothing else.
    if off <= lariat_x {
        state::set_lariat_been_out(entry, true);
    }
    let in_lariat = state::lariat_been_out(entry) && off >= lariat_x && off <= lariat_end_x;
    let f = MotionModule::frame(boma) as i32;
    // EVERY FRAME WHILE THEY ARE IN THE AIR, ten-frame samples otherwise.
    //
    // The last open question about the whip is the one the playtest describes as "the frame where
    // lariat lands mario is mostly behind pikachu already, barely at the edge of the hitbox", and
    // it cannot be answered at ten-frame resolution: the native return leg is NINE frames long. A
    // per-frame `off` during THROWN/ATTACH_ROPE/RETURN is directly comparable to the native
    // recording, which has
    //
    //     f36 THROWN off=-7.09   f44 ATTACH_ROPE off=-41.55   f64 RETURN off=-46.59   f73 off=+3.09
    //
    // i.e. roughly 5.5 units a frame coming home. `special_s_through_target_return_speed` is 2.8,
    // so either the captor closes the other half himself during the lariat or the port is reading
    // the return wrong -- and those two look identical at this resolution and nothing alike at
    // one frame.
    let vst = crate::victim::boma(state::swing_thrown_fighter(entry))
        .map(|v| StatusModule::status_kind(v))
        .unwrap_or(-1);
    let in_flight = vst == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_THROWN
        || vst == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_ATTACH_ROPE
        || vst == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_RETURN;
    if in_flight || f % 10 == 0 {
        crate::telemetry::sink::diag(format!(
            "[rat] WHIP p{entry} f{f} area: off={off:.2} in={in_lariat} out={} \
             lariat={lariat_x:.0} end={lariat_end_x:.0} fail={failure_x:.0} vst={vst:#x} lr={:.0}",
            state::lariat_been_out(entry),
            PostureModule::lr(boma)
        ));
    }
    let was_in = WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_LARIAT_AREA);
    if in_lariat {
        if !was_in {
            // Entering: raise FIRST so the loop records where the victim was. The loop clears it.
            WorkModule::on_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_LARIAT_AREA);
            WorkModule::on_flag(
                boma,
                *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_LARIAT_AREA_FIRST,
            );
        }
    } else if was_in {
        WorkModule::off_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_LARIAT_AREA);
    }

    // BOTH OF THESE ARE GATED ON HAVING BEEN OUT THERE FIRST, and writing them ungated was a bug
    // I introduced and caught by re-reading my own code rather than by testing it.
    //
    // The areas sit at captor-relative x of -24, -10 and -6, and the victim STARTS the throw right
    // next to the captor at off ~ 0. So `off > failure_x` (-6) is true on the very first frame,
    // and ungated it would raise FLAG_FAILURE_AREA immediately -- the THROW loop checks that
    // before anything else and changes status to SPECIAL_S_END, so the whole throw would have
    // ended on frame one. `off > lariat_end_x` (-10) is true on frame one for the same reason,
    // which would close the lariat window before it ever opened and make the lariat unreachable.
    //
    // The interval only means anything on the way BACK. `been_out` records that the victim has
    // reached the lariat area at least once; until then neither line can fire. That matches the
    // shape of the move -- thrown out to the ropes, then the window is open on the return trip,
    // then past -10 it closes and past -6 it is a whiff.
    if in_lariat {
        state::set_lariat_been_out(entry, true);
    }

    // THE ROPE'S ONE REMAINING JOB, SUPPLIED BY THE PORT: turning the victim around.
    //
    // The playtest said "mario just walks away rather than getting thrown into ropes and coming
    // back for a hit", and that is the CORRECT no-rope behaviour of his own status. The ropes are
    // not stage geometry -- they are the article Incineroar generates, which ADR 0004 excludes --
    // so `status_SwingGaogaenThrown_Main` (common 0x710028d180) finds no wall to bounce off:
    // `is_touch(GROUND_TOUCH_FLAG_RIGHT/LEFT)` -> `STOP_WALL` never fires and the loop ends him in
    // WAIT. `crate::victim::trace` recorded exactly that: 0x176 -> 0x177 -> 0x0.
    //
    // But the trip BACK is its own status and it needs nothing from the rope.
    // `status_SwingGaogaenReturn` (0x710028f230) reverses his facing, plays the rope-run motion and
    // drives the STOP energy from `..._WORK_FLOAT_RETURN_SPEED_X` -- which his own CATCHED status
    // already stored from OUR `special_s_through_target_return_speed`. So the port only has to
    // supply the trigger the rope would have supplied, which is "he got out to the ropes".
    //
    // The distance is the game's own: `special_s_lariat_offset_x1` is -24, and the lariat area IS
    // where the ropes are -- that is why pressing inside it is the lariat and why the rate formula
    // measures from its entry point. So reaching -24 is the rope hit.
    //
    // This is the same substitution as the area flags above and it is the last piece of the rope
    // the move actually needs. What is still missing without the article is the rope's visual and
    // the `_over_rope` case, neither of which is a behaviour the follow-ups read.
    // THE MISSING LEG: THROWN -> ATTACH_ROPE -> RETURN, WHICH IS THE SNAP INTO THE ROPES.
    //
    // The port used to go THROWN -> RETURN directly. The native recording does not:
    //
    //     TRACE mario1 st=0x176  ControlModule::start_clatter(85, 0, 5, -1, 0, false, false)
    //     TRACE GAOGAEN0 st=0x1e1  LinkModule::send_event_nodes(link=0, gaogaen_swing_throw, 0)
    //     TRACE mario1 st=0x177  mot=0xbca3f3207  enable_energy(0x3)
    //     TRACE GAOGAEN0 st=0x1e1  LinkModule::send_event_nodes(link=3, 0x2a5eaa0cb9, 0)
    //     TRACE mario1 st=0x178  LinkModule::unlink(link=0)
    //     TRACE mario1 st=0x179  mot=0x35076a4c0  enable_energy(0x3)
    //
    // Two events, and the second goes to link 3 -- the ROPE, not the victim. The rope receives it
    // and puts the victim into 0x178, which is `SWING_GAOGAEN_ATTACH_ROPE`: `change_motion`,
    // `set_rate` and `GroundModule::correct`, the visual of being caught by the ropes. Then the
    // rope moves them on to RETURN. This port has no rope to receive that event, so it supplies
    // both halves -- the same substitution as the trigger itself, and the reason
    // `FORBIDDEN_VICTIM_STATUSES` is now empty is that all three of those functions were
    // disassembled and none of them touches a rope.
    //
    // ONE FRAME IN ATTACH_ROPE, AND THAT IS A MEASUREMENT RATHER THAN A CHOICE. Its `Main` has no
    // `change_status` in it at all, so it cannot decide its own length -- natively the rope's next
    // event is what ends it, and in the recording the victim's 0x178 and 0x179 entry traces are
    // adjacent lines. Holding them there for a duration would mean inventing one.
    whip_flight(fighter, entry, off);
    if state::lariat_been_out(entry) && !in_lariat {
        if off > lariat_end_x {
            WorkModule::on_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_LARIAT_END_AREA);
        }
        if off > failure_x {
            WorkModule::on_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_FAILURE_AREA);
        }
    }
}

unsafe extern "C" fn special_s_throw_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);

    // THE VICTIM'S OWN STATE, ONE LINE PER CHANGE. `crate::victim::trace` is the instrument the
    // cargo has used since round four and the whip never did -- which is why three rounds of whip
    // reports were about the victim while every log line was about us. It answers the question the
    // last playtest could not: whether the forced SWING_GAOGAEN_CATCHED actually takes, and
    // whether their clatter starts at our `special_s_catch_clatter_frame` of 85 or at zero.
    if let Some(vb) = crate::victim::boma(state::swing_thrown_fighter(entry)) {
        crate::victim::trace(entry, vb, "whip");
    }

    // NOT PART OF THE TRANSCRIPTION -- the rope would be doing this. See `whip_update_areas`.
    whip_update_areas(fighter);

    if !whip_victim_held(boma) {
        // The rope's own retirement is the one call ADR 0004 removes; the transition is not.
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_END.into(), false.into());
        return 0.into();
    }

    // Dragged along the ropes: the ACMD raises CHECK_WALL_MOVE and parks the destination in
    // FLOAT_GAOGAEN_TARGET_X, and this pins x to it every frame while keeping y and z.
    if WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_CHECK_WALL_MOVE) {
        let mut pos: Vector3f = *PostureModule::pos(boma);
        pos.x = WorkModule::get_float(
            boma,
            *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLOAT_GAOGAEN_TARGET_X,
        );
        PostureModule::set_pos(boma, &pos);
    }

    // The throw itself: one event to the victim, one to the rope. The rope event is the only hash
    // in `docs/the-recording.md` that never resolved, and under ADR 0004 there is no rope article
    // to receive it, so only the victim's event is sent.
    if WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_THROW_ROPE) {
        WorkModule::off_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_THROW_ROPE);
        WorkModule::on_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_THROW_ROPE_END);
        LinkModule::send_event_nodes(
            boma,
            *LINK_NO_CAPTURE,
            Hash40::new("gaogaen_swing_throw"),
            0,
        );
    }

    // THE FOLLOW-UP WINDOW OPENS WHEN `THROW_ROPE_END` IS SET, AND THIS PORT HAD THE TEST
    // INVERTED. That one `!` is why neither follow-up has ever happened.
    //
    // Straight out of `SpecialSThrow_main_loop` (gaogaen), and the branch polarity is read from
    // Ghidra's own `if ((bVar2 & 1U) == 0)`, which is the FALSE arm:
    //
    //     if is_flag(SPECIAL_S_FLAG_THROW_ROPE_END) then
    //         if not (180.0 <= globalTable[0xe]) then
    //             if is_flag(FAILURE_AREA) then change_status(SPECIAL_S_FAILURE, true)
    //             elseif is_flag(LARIAT_AREA) and is_flag(LARIAT_END_AREA) == false then ...
    //     else
    //         <the situation swap and the motion tail>
    //     end
    //
    // `THROW_ROPE_END` means "the victim is on their way to the ropes" -- it is raised, a dozen
    // lines above this, on the same frame the throw event goes out. So gating the follow-ups on
    // its ABSENCE made them reachable only before the throw and unreachable for the entire flight.
    // The measured log is exactly that shape: the throw event at f30 raises the flag, the victim
    // arrives in the lariat window at f60 with `in=true`, and Pikachu goes
    // `RAT_SPECIAL_S_THROW(0x1f4) -> 0x1f5` -- SPECIAL_S_END -- without ever looking.
    //
    //     [rat] TRACE pika0 st=0x1f4 ... LinkModule::send_event_nodes(event=0x13b7318a5a)
    //     [rat] WHIP p0 f60 area: off=-21.89 in=true out=true lariat=-24 end=-10 fail=-6
    //     [rat] pika0 motion 0x0fec894555 ended after 89f
    //
    // AND THE FAILURE BRANCH WAS THE WRONG STATUS. It went to `SPECIAL_S_END` with `false`;
    // Incineroar changes to `SPECIAL_S_FAILURE` with `true`. SPECIAL_S_FAILURE is a status this
    // port already ships, with its own motion -- the whiff animation -- and END is the ordinary
    // recovery. Sending a whiff to END skips the animation that tells the player they missed.
    //
    // The 180-frame guard is `_ZNK3lib8L2CValueleERKS0_` at gaogaen 0x71000229ec, the only
    // ordering operator in the whole function, so there is no ambiguity about which comparison it
    // is. `global_table[0xe]` is the status frame, as `whip_followup_loop` already reads it.
    let window_open =
        WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_THROW_ROPE_END)
            && !(180.0 <= fighter.global_table[0xe].get_f32());
    if window_open {
        if WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_FAILURE_AREA) {
            crate::telemetry::sink::diag(format!(
                "[rat] WHIP p{entry} the victim came back past the failure line -- SPECIAL_S_FAILURE"
            ));
            fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_FAILURE.into(), true.into());
            return 0.into();
        }

        let pressed = ControlModule::check_button_on(boma, *CONTROL_PAD_BUTTON_SPECIAL)
            || ControlModule::check_button_on(boma, *CONTROL_PAD_BUTTON_ATTACK);

        if WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_LARIAT_AREA)
            && !WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_LARIAT_END_AREA)
        {
            // First frame inside the area: remember where the victim was.
            if WorkModule::is_flag(
                boma,
                *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_LARIAT_AREA_FIRST,
            ) {
                WorkModule::off_flag(
                    boma,
                    *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_LARIAT_AREA_FIRST,
                );
                if let Some(x) = victim_pos_x(boma) {
                    state::set_lariat_area_hit_pos_x(entry, x);
                }
            }

            if WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_SHOULDER) {
                // The press happened EARLIER, outside the area. This is the body slam.
                fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_SHOULDER.into(), true.into());
                return 0.into();
            }
            if pressed {
                // The press happened HERE, inside the area. This is the lariat, and where it
                // happened sets the animation speed.
                if let Some(x) = victim_pos_x(boma) {
                    state::set_lariat_button_push_pos_x(entry, x);
                }
                fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_LARIAT.into(), true.into());
                return 0.into();
            }
        }

        // Outside the area a press is REMEMBERED, not acted on. This one line is the whole of
        // "press early and he slams them instead".
        if pressed {
            WorkModule::on_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_SHOULDER);
        }
    }

    // The same situation-swap-and-end tail every whip loop has. The source's `is_exist(ROPE)` guard
    // on the final transition is vacuous here for the ADR 0004 reason, so the motion ending is the
    // whole condition.
    if !MotionModule::is_end(boma) {
        let prev_ground = fighter.global_table[0x17] == *SITUATION_KIND_GROUND;
        let now_ground = fighter.global_table[0x16] == *SITUATION_KIND_GROUND;
        if !prev_ground && now_ground {
            GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
            MotionModule::change_motion_inherit_frame(
                boma, Hash40::new("special_s_throw"), -1.0, 1.0, 0.0, false, false,
            );
            whip_kinetics(fighter, true);
        } else if prev_ground && !now_ground {
            GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
            MotionModule::change_motion_inherit_frame(
                boma, Hash40::new("special_air_s_throw"), -1.0, 1.0, 0.0, false, false,
            );
            whip_kinetics(fighter, true);
        }
    } else {
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_END.into(), false.into());
    }

    0.into()
}

/// The held victim's world x, or `None` if there is no live victim.
///
/// The source reaches the victim with
/// `BattleObjectManager::is_active_find_battle_object(get_int(SWING_THROWN_FIGHTER))` and then
/// `PostureModule::pos_x` on their accessor. Routed through the port's own stored id and
/// `crate::victim`, which already owns "is this cross-object handle safe to touch" -- see
/// `tools/check.sh` step 4d, which requires every cross-object handle to go through it.
unsafe fn victim_pos_x(boma: *mut BattleObjectModuleAccessor) -> Option<f32> {
    let id = state::swing_thrown_fighter(entry_of(boma));
    if !sv_battle_object::is_active(id) {
        return None;
    }
    let vboma = crate::victim::boma(id)?;
    Some(PostureModule::pos_x(vboma))
}

/// SPECIAL_S `status_end`, gaogaen `0x71000044d0`. It is FOUR INSTRUCTIONS and does nothing:
///
///     mov x0, x8 ; mov w1, wzr ; b lib::L2CValue     -- i.e. `return L2CValue(false)`
///
/// Worth stating because `tools/disasm --trace` reports this status_end as containing an
/// `init_settings` and a `set_fighter_status_data`. It does not: SPECIAL_S's end is 16 bytes and
/// SPECIAL_S_THROW's pre begins immediately after, so the trace ran straight off the end of one
/// function into the next. The two entries in `work/whip-source.txt` that share the addresses
/// 0x7100004614 and 0x7100004768 are the same function body seen twice, and transcribing the end
/// from that reading would have put a whole status setup where there is a bare return.
unsafe extern "C" fn special_s_end(_fighter: &mut L2CFighterCommon) -> L2CValue {
    false.into()
}

/// SPECIAL_S_THROW `status_end`, gaogaen `0x7100004ba0`.
///
/// Retires the rope article, then runs the release sequence -- which the source INLINES here
/// rather than calling `0x71000206c0`, instruction for instruction identical to it. Transcribed as
/// the shared `whip_release` on purpose: one copy in the port for one behaviour in the game.
/// THE `change_status_exist` CALL IS OMITTED UNDER ADR 0004. It retires gaogaen's rope article,
/// which this port does not ship and cannot -- `tools/check.sh` step 4d fails the build if any code
/// requests `SWING_GAOGAEN_THROWN`, `_ATTACH_ROPE` or `_RETURN`. There is no article to retire, so
/// there is nothing for the call to do; it is not a gap in knowledge, it is a consequence of a
/// decision already made and checked.
unsafe fn special_s_throw_end_body(fighter: &mut L2CFighterCommon, line: &str) -> L2CValue {
    if !end_line_first(fighter, line) {
        return false.into();
    }
    whip_release(fighter);
    false.into()
}

unsafe extern "C" fn special_s_throw_end(f: &mut L2CFighterCommon) -> L2CValue {
    special_s_throw_end_body(f, "End")
}

unsafe extern "C" fn special_s_throw_exit(f: &mut L2CFighterCommon) -> L2CValue {
    special_s_throw_end_body(f, "Exit")
}

/// SPECIAL_S_SHOULDER -- the body slam. Source pre `0x7100005170`, main `0x7100005550`,
/// end `0x7100005820`.
unsafe extern "C" fn shoulder_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    whip_pre(fighter, *FIGHTER_STATUS_ATTR_DISABLE_TURN_DAMAGE)
}

/// SOURCE, `0x7100005550`, in order:
///
///     if situation == GROUND { correct(GROUND, GROUND_CLIFF_STOP); change_motion("special_s_shoulder") }
///     else                   { correct(AIR);                       change_motion("special_air_s_shoulder") }
///     0x710001d4b0(agent, true)
///     sub_shift_status_main(<loop>)
unsafe extern "C" fn shoulder_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
        MotionModule::change_motion(
            boma, Hash40::new("special_s_shoulder"), 0.0, 1.0, false, 0.0, false, false,
        );
    } else {
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
        MotionModule::change_motion(
            boma, Hash40::new("special_air_s_shoulder"), 0.0, 1.0, false, 0.0, false, false,
        );
    }
    whip_kinetics(fighter, true);
    fighter.sub_shift_status_main(L2CValue::Ptr(followup_main_loop as *const () as _))
}

unsafe fn shoulder_end_body(fighter: &mut L2CFighterCommon, line: &str) -> L2CValue {
    if !end_line_first(fighter, line) {
        return false.into();
    }
    whip_release(fighter);
    false.into()
}

unsafe extern "C" fn shoulder_end(f: &mut L2CFighterCommon) -> L2CValue {
    shoulder_end_body(f, "End")
}

unsafe extern "C" fn shoulder_exit(f: &mut L2CFighterCommon) -> L2CValue {
    shoulder_end_body(f, "Exit")
}

/// SPECIAL_S_LARIAT -- the spinning throw. Source pre `0x7100005870`, main `0x7100005c50`,
/// end `0x71000062c0`.
unsafe extern "C" fn lariat_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    whip_pre(fighter, *FIGHTER_STATUS_ATTR_DISABLE_TURN_DAMAGE)
}

/// The only follow-up that is not just motion-plus-kinetics: its ANIMATION SPEED is interpolated
/// by how far into the lariat window the button was pressed.
///
/// SOURCE, `0x7100005c50`, after the motion change:
///
///     dist = math_abs(get_float(LARIAT_BUTTON_PUSH_POS_X) - get_float(LARIAT_AREA_HIT_POS_X))
///     len  = get_float(LARIAT_AREA_LENGTH)
///     t    = 1.0
///     if 0 < len { t = dist / len }
///     lo   = param_special_s.special_s_lariat_motion_rate_min
///     hi   = param_special_s.special_s_lariat_motion_rate_max
///     MotionModule::set_rate(lo + (hi - lo) * t)
///
/// THE THREE FLOATS COME FROM `state`, NOT FROM `WorkModule`. They are gaogaen instance-work
/// floats 0x50, 0x51 and 0x52, and Pikachu's `FIGHTER_INSTANCE_WORK_ID_FLOAT_TERM` is 0x4C -- so
/// unlike most of the collisions in this port these would be a genuine out-of-bounds write rather
/// than a silent alias. See the header of `state.rs`.
unsafe extern "C" fn lariat_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);

    if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
        MotionModule::change_motion(
            boma, Hash40::new("special_s_lariat"), 0.0, 1.0, false, 0.0, false, false,
        );
    } else {
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
        MotionModule::change_motion(
            boma, Hash40::new("special_air_s_lariat"), 0.0, 1.0, false, 0.0, false, false,
        );
    }

    let dist = (state::lariat_button_push_pos_x(entry) - state::lariat_area_hit_pos_x(entry)).abs();
    let len = state::lariat_area_length(entry);
    let t = if len > 0.0 { dist / len } else { 1.0 };
    let lo = WorkModule::get_param_float(
        boma,
        hash40("param_special_s"),
        hash40("special_s_lariat_motion_rate_min"),
    );
    let hi = WorkModule::get_param_float(
        boma,
        hash40("param_special_s"),
        hash40("special_s_lariat_motion_rate_max"),
    );
    MotionModule::set_rate(boma, lo + (hi - lo) * t);

    whip_kinetics(fighter, true);
    fighter.sub_shift_status_main(L2CValue::Ptr(followup_main_loop as *const () as _))
}

unsafe fn lariat_end_body(fighter: &mut L2CFighterCommon, line: &str) -> L2CValue {
    if !end_line_first(fighter, line) {
        return false.into();
    }
    whip_release(fighter);
    false.into()
}

unsafe extern "C" fn lariat_end(f: &mut L2CFighterCommon) -> L2CValue {
    lariat_end_body(f, "End")
}

unsafe extern "C" fn lariat_exit(f: &mut L2CFighterCommon) -> L2CValue {
    lariat_end_body(f, "Exit")
}

/// SPECIAL_S_FAILURE -- the whiff. Source pre `0x7100006310`, main `0x71000066f0`,
/// end `0x7100006ab0`.
unsafe extern "C" fn failure_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    whip_pre(fighter, *FIGHTER_STATUS_ATTR_DISABLE_TURN_DAMAGE)
}

/// SOURCE, `0x71000066f0`: motion, kinetics, and then INCINEROAR HURTS HIMSELF FOR MISSING --
/// `special_s_fail_damage` out of his own param, applied with `DamageModule::add_damage`, gated on
/// `HitModule::get_total_status` being HIT_STATUS_NORMAL so it does not land while he is invincible
/// or already in hitlag.
unsafe extern "C" fn failure_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
        MotionModule::change_motion(
            boma, Hash40::new("special_s_failure"), 0.0, 1.0, false, 0.0, false, false,
        );
    } else {
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
        MotionModule::change_motion(
            boma, Hash40::new("special_air_s_failure"), 0.0, 1.0, false, 0.0, false, false,
        );
    }
    whip_kinetics(fighter, true);

    if HitModule::get_total_status(boma, 0) as i32 == *HIT_STATUS_NORMAL {
        let damage = WorkModule::get_param_float(
            boma,
            hash40("param_special_s"),
            hash40("special_s_fail_damage"),
        );
        DamageModule::add_damage(boma, damage, 0);
    }

    fighter.sub_shift_status_main(L2CValue::Ptr(followup_main_loop as *const () as _))
}

/// SOURCE, `0x7100006ab0`: the release, and then -- uniquely among the six -- IT TURNS HIM AROUND.
///
///     0x71000206c0(agent)
///     if <situation test> { PostureModule::reverse_lr(); PostureModule::update_rot_y_lr() }
/// THE TURN-AROUND HAS NEVER HAPPENED EITHER, for the same reason the release never has.
///
/// This is the second casualty of the dead `End` line and it is the one you can see: Incineroar
/// ends a whiffed whip facing the other way, and modified Pikachu has been shrugging and staying
/// put through every one of the 24 failures in pt22. It comes back with the release, on the line
/// that arrives.
unsafe fn failure_end_body(fighter: &mut L2CFighterCommon, line: &str) -> L2CValue {
    if !end_line_first(fighter, line) {
        return false.into();
    }
    let boma = fighter.module_accessor;
    whip_release(fighter);
    // `next == DOWN_SPOT`, NOT `situation == GROUND`, AND THE PLAYTEST CONFIRMED IT BEFORE THE
    // DECOMP DID: "Pikachu now behaves correctly after a missed whip, no turn around."
    //
    // `SpecialSFailure_end` is two statements -- the shared release, then:
    //
    //     if globalTable[0xb] == FIGHTER_STATUS_KIND_DOWN_SPOT {
    //         PostureModule::reverse_lr(); PostureModule::update_rot_y_lr()
    //     }
    //
    // So Incineroar turns around only when the whiff puts him on the FLOOR, not on every grounded
    // whiff. Reading it as a situation test would have spun Pikachu on his heel after every shrug,
    // which is a thing Incineroar does not do.
    if fighter.global_table[0xb] == *FIGHTER_STATUS_KIND_DOWN_SPOT {
        PostureModule::reverse_lr(boma);
        PostureModule::update_rot_y_lr(boma);
    }
    false.into()
}

unsafe extern "C" fn failure_end(f: &mut L2CFighterCommon) -> L2CValue {
    failure_end_body(f, "End")
}

unsafe extern "C" fn failure_exit(f: &mut L2CFighterCommon) -> L2CValue {
    failure_end_body(f, "Exit")
}

/// SPECIAL_S_END -- the common exit. Source pre `0x7100006b70`, main `0x7100006f50`,
/// end `0x7100007380`.
///
/// Its pre is the ONLY one of the six whose `set_fighter_status_data` passes no status attr at all:
/// no START_TURN, no DISABLE_TURN_DAMAGE.
unsafe extern "C" fn end_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    whip_pre(fighter, 0)
}

/// SOURCE, `0x7100006f50`: motion, kinetics, then a last chance to drop a victim who is somehow
/// still attached -- the same parent-id-against-SWING_THROWN_FIGHTER test the release helper opens
/// with, without the node half.
unsafe extern "C" fn end_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
        MotionModule::change_motion(
            boma, Hash40::new("special_s_end"), 0.0, 1.0, false, 0.0, false, false,
        );
    } else {
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
        MotionModule::change_motion(
            boma, Hash40::new("special_air_s_end"), 0.0, 1.0, false, 0.0, false, false,
        );
    }
    whip_kinetics(fighter, true);

    let link = *LINK_NO_CAPTURE;
    if LinkModule::is_link(boma, link) {
        let parent = LinkModule::get_parent_id(boma, link, false);
        // `state`, not `WorkModule` -- see `state.rs`. Gaogaen's
        // `..._INT_BATTLE_OBJECT_ID_SWING_THROWN_FIGHTER` is 0x100000C0, which lands inside
        // Pikachu's own int block (his INT_TERM is 0x100000C2), so writing it there is a silent
        // alias rather than an error.
        let thrown = state::swing_thrown_fighter(entry_of(boma));
        if parent as u32 == thrown {
            LinkModule::unlink(boma, link);
        }
    }

    fighter.sub_shift_status_main(L2CValue::Ptr(followup_main_loop as *const () as _))
}

/// SOURCE, `0x7100007380`: retire the rope and nothing else -- so under ADR 0004 this status_end
/// has nothing to do at all, and is registered only so the status is completely owned.
unsafe extern "C" fn end_end(_fighter: &mut L2CFighterCommon) -> L2CValue {
    false.into()
}

/// IS THE FIGHTER WE CAUGHT STILL THERE? -- gaogaen `0x7100021910`.
///
/// A watchdog, and the reason the whip does not leave a victim welded on when something unusual
/// happens to them. It re-reads the stored id every frame and asks four separate questions: the id
/// is not INVALID, the object is not null, the object is active, and it is a FIGHTER currently in
/// one of the four `SWING_GAOGAEN_*` victim statuses. Anything else and the caller tears the link
/// down.
///
/// ALL FOUR STATUSES ARE TESTED, INCLUDING THE THREE ADR 0004 EXCLUDES. `_ATTACH_ROPE`, `_RETURN`
/// and `_THROWN` belong to the rope flight this port does not implement, so those comparisons can
/// never match here -- but they are read-only comparisons, not requests, so this is a literal
/// transcription and `tools/check-victim.sh` is satisfied by construction: it greps for the
/// constants inside a `change_status` call, which is the thing that actually crashes.
unsafe fn whip_victim_held(boma: *mut BattleObjectModuleAccessor) -> bool {
    let id = state::swing_thrown_fighter(entry_of(boma));
    if id == *BATTLE_OBJECT_ID_INVALID as u32 {
        return false;
    }
    if sv_battle_object::is_null(id) {
        return false;
    }
    if !sv_battle_object::is_active(id) {
        return false;
    }
    if sv_battle_object::category(id) != *BATTLE_OBJECT_CATEGORY_FIGHTER {
        return false;
    }
    // Through `crate::victim::boma`, which is the ONLY sanctioned way to turn an id into another
    // object's accessor here -- it re-checks plausibility, null and that the accessor's own
    // `battle_object_id` matches what was asked for. `tools/check-victim.sh` fails the build on a
    // raw `sv_battle_object::module_accessor`, and it caught this exact line.
    let Some(vboma) = crate::victim::boma(id) else {
        return false;
    };
    let st = StatusModule::status_kind(vboma);
    st == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED
        || st == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_ATTACH_ROPE
        || st == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_RETURN
        || st == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_THROWN
}

/// Hands gravity back, gaogaen `0x710001fc90`. Self-gating on its own flag, so it is safe to call
/// every frame -- which is what the source does.
unsafe fn whip_air_gravity(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    if !WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_ENABLE_GRAVITY) {
        return;
    }
    WorkModule::off_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_ENABLE_GRAVITY);
    let gravity = *FIGHTER_KINETIC_ENERGY_ID_GRAVITY;
    // The two situations do DIFFERENT things, not one thing conditionally: in the air gravity is
    // reset and switched on, on the ground it is only un-disabled. Worth being exact about --
    // resetting gravity on a grounded fighter is how a move ends up popping him off the floor.
    if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
        KineticUtility::clear_unable_energy(gravity, boma);
    } else {
        reset_energy(fighter, gravity, *ENERGY_GRAVITY_RESET_TYPE_GRAVITY, 0.0);
        KineticModule::enable_energy(boma, gravity);
    }
}

/// AFTER A WHIP YOU DRIFT LESS, and this is where that comes from -- gaogaen `0x710001ff80`.
///
/// Nothing in the recording hinted at this and nothing about the move's description implies it: the
/// follow-up restores air control with the fighter's ordinary air stats MULTIPLIED by two
/// whip-specific params.
///
///     accel_x_mul = air_accel_x_mul    * special_s_attack_after_control_speed_x_mul
///     accel_x_add = air_accel_x_add    * special_s_attack_after_control_speed_x_mul
///     stable      = air_speed_x_stable * special_s_attack_after_control_speed_x_stable
///
/// `air_accel_x_mul`, `air_accel_x_add` and `air_speed_x_stable` are read with NO parent param --
/// they are the fighter's own attributes out of `fighter_param`, so on a Pikachu they are PIKACHU's
/// air stats, scaled by Incineroar's after-attack multipliers. That is the correct reading of the
/// source and also the behaviour this moveset wants: Pikachu's mobility, Incineroar's move.
unsafe fn whip_air_control(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    if !WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_WORK_ID_FLAG_AIR_CONTROL) {
        return;
    }
    // Tested against SITUATION_KIND_AIR explicitly, not "not ground" -- there is a third situation
    // (CLIFF) and the source does not run this in it.
    if fighter.global_table[0x16] != *SITUATION_KIND_AIR {
        return;
    }
    let control = *FIGHTER_KINETIC_ENERGY_ID_CONTROL;
    reset_energy(
        fighter,
        control,
        *ENERGY_CONTROLLER_RESET_TYPE_FALL_ADJUST,
        0.0,
    );
    KineticModule::enable_energy(boma, control);

    let after_mul = WorkModule::get_param_float(
        boma,
        hash40("param_special_s"),
        hash40("special_s_attack_after_control_speed_x_mul"),
    );
    let accel_mul = WorkModule::get_param_float(boma, hash40("air_accel_x_mul"), 0) * after_mul;
    let accel_add = WorkModule::get_param_float(boma, hash40("air_accel_x_add"), 0) * after_mul;
    fighter.agent.clear_lua_stack();
    fighter.agent.push_lua_stack(&mut L2CValue::new_num(accel_mul));
    sv_kinetic_energy::controller_set_accel_x_mul(fighter.lua_state_agent);
    fighter.agent.clear_lua_stack();
    fighter.agent.push_lua_stack(&mut L2CValue::new_num(accel_add));
    sv_kinetic_energy::controller_set_accel_x_add(fighter.lua_state_agent);

    let stable = WorkModule::get_param_float(boma, hash40("air_speed_x_stable"), 0)
        * WorkModule::get_param_float(
            boma,
            hash40("param_special_s"),
            hash40("special_s_attack_after_control_speed_x_stable"),
        );
    fighter.agent.clear_lua_stack();
    fighter.agent.push_lua_stack(&mut L2CValue::new_int(control as u64));
    fighter.agent.push_lua_stack(&mut L2CValue::new_num(stable));
    sv_kinetic_energy::set_stable_speed(fighter.lua_state_agent);

    WorkModule::off_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_WORK_ID_FLAG_AIR_CONTROL);
}

/// THE PER-FRAME LOOP SHARED BY SHOULDER, LARIAT, FAILURE AND END.
///
/// The game has four of these -- `0x7100021d00`, `0x7100020fc0`, `0x7100020aa0`, `0x710001f770` --
/// and their heads and tails are instruction-for-instruction identical. They differ only in the two
/// motion names and in whether they carry a body. So this is one function branching on status, for
/// the same reason `whip_kinetics` is: four summaries of one shape is how the old implementation
/// drifted.
///
/// THE TRANSITIONS ARE THE GAME'S OWN, WHICH IS THE POINT. `sub_transition_group_check_air_cliff`,
/// `sub_wait_ground_check_common` and `sub_air_check_fall_common` are `L2CFighterCommon` methods
/// compiled into the engine; the source calls them and so does this. Nothing here reimplements
/// "when should the whip end" -- it asks the same three questions in the same order and honours the
/// same answers.
///
/// THE COMMON CHECKS ONLY RUN WHEN THE ANIMATION IS CANCELLABLE. `is_enable_cancel` gates them,
/// which is not obvious and matters: before that point the fighter cannot be pulled out into wait
/// or fall no matter what the ground says.
unsafe fn whip_followup_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let status = StatusModule::status_kind(boma);

    // SPECIAL_S's OWN main_loop IS THIS SAME SKELETON -- `0x71000239d0` is instruction-for-
    // instruction the follow-ups' loop with the start motions, no victim watchdog, and no gravity
    // restore. Five of the six loops are one function.
    let (mot_ground, mot_air) = if status == *FIGHTER_STATUS_KIND_SPECIAL_S {
        ("special_s_start", "special_air_s_start")
    } else if status == RAT_STATUS_KIND_SPECIAL_S_SHOULDER {
        ("special_s_shoulder", "special_air_s_shoulder")
    } else if status == RAT_STATUS_KIND_SPECIAL_S_LARIAT {
        ("special_s_lariat", "special_air_s_lariat")
    } else if status == RAT_STATUS_KIND_SPECIAL_S_FAILURE {
        ("special_s_failure", "special_air_s_failure")
    } else {
        ("special_s_end", "special_air_s_end")
    };

    if fighter.sub_transition_group_check_air_cliff().get_bool() {
        return 1.into();
    }
    if CancelModule::is_enable_cancel(boma) {
        if fighter.sub_wait_ground_check_common(false.into()).get_bool() {
            return 1.into();
        }
        if fighter.sub_air_check_fall_common().get_bool() {
            return 1.into();
        }
    }

    // SHOULDER and LARIAT only: drop a victim who has stopped being a victim. FAILURE never had
    // one and END has already let go.
    if status == RAT_STATUS_KIND_SPECIAL_S_SHOULDER || status == RAT_STATUS_KIND_SPECIAL_S_LARIAT {
        if LinkModule::is_link(boma, *LINK_NO_CAPTURE)
            && !WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_HIT)
            && !whip_victim_held(boma)
        {
            // `whip_release` returns false when it bailed out early and true when it actually tore
            // the link down; only then is the stored id stale and worth clearing.
            // The watchdog calls the BODY: see `whip_release_teardown`. It has already decided,
            // from the three conditions above, that the victim is stale.
            {
                whip_release_teardown(fighter);
                state::set_swing_thrown_fighter(entry_of(boma), *BATTLE_OBJECT_ID_INVALID as u32);
            }
        }
    }

    if !MotionModule::is_end(boma) {
        // LANDING OR LEAVING THE GROUND MID-ANIMATION swaps to the matching motion and KEEPS THE
        // FRAME -- `change_motion_inherit_frame`, not `change_motion` -- then re-runs the kinetics
        // for the new situation. Without this the follow-up restarts its animation on every
        // transition, which is what "it visually looks like the character models are off" was.
        let prev_ground = fighter.global_table[0x17] == *SITUATION_KIND_GROUND;
        let now_ground = fighter.global_table[0x16] == *SITUATION_KIND_GROUND;
        if !prev_ground && now_ground {
            GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
            MotionModule::change_motion_inherit_frame(
                boma, Hash40::new(mot_ground), -1.0, 1.0, 0.0, false, false,
            );
            whip_kinetics(fighter, true);
        } else if prev_ground && !now_ground {
            GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
            MotionModule::change_motion_inherit_frame(
                boma, Hash40::new(mot_air), -1.0, 1.0, 0.0, false, false,
            );
            whip_kinetics(fighter, true);
        }
        // SPECIAL_S calls only the air-control half. The gravity restore is a follow-up thing:
        // during the catch itself gravity is still being held off deliberately.
        if status != *FIGHTER_STATUS_KIND_SPECIAL_S {
            whip_air_gravity(fighter);
        }
        whip_air_control(fighter);
    } else if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
        fighter.change_status((*FIGHTER_STATUS_KIND_WAIT).into(), false.into());
    } else {
        fighter.change_status((*FIGHTER_STATUS_KIND_FALL).into(), false.into());
    }

    0.into()
}

unsafe extern "C" fn followup_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    whip_followup_loop(fighter)
}

/// All six statuses, pre / main / end, each transcribed from the address named on it.
///
/// Registering this REPLACES `specials/specials/mod.rs` for the whip. The two must never be
/// installed at once -- whichever registered last would silently own the status, and "which
/// implementation am I testing" is not a question worth having.
pub fn install() {
    Agent::new("pikachu")
        .status(Pre, *FIGHTER_STATUS_KIND_SPECIAL_S, special_s_pre)
        .status(Main, *FIGHTER_STATUS_KIND_SPECIAL_S, special_s_main)
        .status(End, *FIGHTER_STATUS_KIND_SPECIAL_S, special_s_end)
        .status(Pre, RAT_STATUS_KIND_SPECIAL_S_THROW, special_s_throw_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_THROW, special_s_throw_main)
        .status(End, RAT_STATUS_KIND_SPECIAL_S_THROW, special_s_throw_end)
        .status(Exit, RAT_STATUS_KIND_SPECIAL_S_THROW, special_s_throw_exit)
        .status(Pre, RAT_STATUS_KIND_SPECIAL_S_SHOULDER, shoulder_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_SHOULDER, shoulder_main)
        .status(End, RAT_STATUS_KIND_SPECIAL_S_SHOULDER, shoulder_end)
        .status(Exit, RAT_STATUS_KIND_SPECIAL_S_SHOULDER, shoulder_exit)
        .status(Pre, RAT_STATUS_KIND_SPECIAL_S_LARIAT, lariat_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_LARIAT, lariat_main)
        .status(End, RAT_STATUS_KIND_SPECIAL_S_LARIAT, lariat_end)
        .status(Exit, RAT_STATUS_KIND_SPECIAL_S_LARIAT, lariat_exit)
        .status(Pre, RAT_STATUS_KIND_SPECIAL_S_FAILURE, failure_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_FAILURE, failure_main)
        .status(End, RAT_STATUS_KIND_SPECIAL_S_FAILURE, failure_end)
        .status(Exit, RAT_STATUS_KIND_SPECIAL_S_FAILURE, failure_exit)
        .status(Pre, RAT_STATUS_KIND_SPECIAL_S_END, end_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_END, end_main)
        .status(End, RAT_STATUS_KIND_SPECIAL_S_END, end_end)
        .install();
}

/// WHERE THE ROPES ARE, in the captor's facing frame at the moment of the throw.
///
/// MEASURED, LIKE `ROPE_HOLD_FRAMES`, AND FOR THE SAME REASON. The ropes belong to gaogaen's
/// ROPE article, which ADR 0004 excludes, so their position is not in `param_special_s` -- the
/// whole object is dumped in `tools/prc-tool` output and there is no rope entry in it. What there
/// is instead is 8070 frames of traced native Incineroar, and the outbound leg in them has ZERO
/// variance across 58 whips:
///
///     victim st  frame  off        delta
///     0x176      19     +1.49              last CATCHED frame, being dragged in
///     0x177      0      -7.09              THROWN begins, always here
///     0x177      1      -18.29     -11.20  = special_s_through_target_speed
///     0x177      2      -29.49     -11.20
///     0x177      3      -40.69     -11.20  <- the ropes
///     0x178      0      -41.55             ATTACH_ROPE
///     0x178      -      -48.15      -6.60  = four frames of _speed_over_rope (1.6)
///     0x178      -      -48.32 .. -49.39   a decaying spring tail, 1.24 in total
///     0x179      1      -46.59     +2.80   = special_s_through_target_return_speed
///
/// Every number in that table except the two `off` extremes is a param this port already reads.
/// `-40.69` is exactly `-7.09 - 3 * 11.20`, so the leg is four frames of the game's own speed and
/// the constant is really "four frames" written as a distance -- which is the form the trigger
/// needs, because comparing a world x cannot drift with a captor who is still sliding.
///
/// THE PORT WAS USING `special_s_lariat_offset_x1` (-24) FOR THIS, and that is the reported bug:
/// -24 puts the line between the second and third frame of the leg, so the victim attached at
/// -29.49 instead of -40.69. One frame short of the ropes, every time. "It goes 4 tiles in the
/// training room instead of 5."
const ROPE_OFFSET_X: f32 = -40.69;

/// How far the ropes give before they stop, measured from `ROPE_OFFSET_X`.
///
/// The 6.60 and the 1.24 spring tail in the table above, which together put the native extreme at
/// off = -48.53. The engine produces this leg ITSELF once the victim is past
/// `SPECIAL_S_FLOAT_ROPE_X` -- it drops them from `_speed` to `_speed_over_rope` -- so this is not
/// a motion this port drives, it is only where it stops.
///
/// AND THE VARIANCE WAS THE MISSING STOP, NOT THE CREEP. Before the rope was a place at all the
/// port let that 1.6 run until the hold timer happened to fire, and the same whip finished
/// anywhere from -48.69 to -58.29: "modified pikachu sometimes throws farther". The creep is
/// deterministic; what was not was when anybody told it to end.
///
/// AND THE PORT PAYS IT OUT ITSELF NOW, in `whip_flight`. The round that added this constant
/// assumed the engine would produce the slow leg once the victim passed `SPECIAL_S_FLOAT_ROPE_X`,
/// and recorded the ATTACH_ROPE animation as a known gap. pt23 measured both assumptions false:
/// the victim took FIVE full 11.20 steps to -51.89 where native takes four to -40.69, and then
/// sat at -51.89 for every one of the twenty hold frames. The engine has the number -- `rope_x`
/// and `GENERATED_ROPE` are both in the param log -- and does not act on it in time.
///
/// So the attach happens at the line, which is native's timing and native's position, and the
/// give is paid out one `_speed_over_rope` per frame inside ATTACH_ROPE, which is native's status
/// as well. The gap is closed rather than recorded.
const ROPE_STRETCH_X: f32 = 7.84;

/// How long the ropes hold the victim, in captor animation frames.
///
/// THE ONE NUMBER IN THE WHIP THAT IS MEASURED RATHER THAN READ. It is not in `param_special_s` --
/// the whole object is dumped in `tools/prc-tool` output and there is no rope entry in it -- because
/// it belongs to the rope ARTICLE, which ADR 0004 excludes. So it comes from three native whips
/// that agree to the frame:
///
///     NATIVE captor st=0x1e1 f=44 | victim st=0x178 off=-41.55
///     NATIVE captor st=0x1e1 f=64 | victim st=0x179 off=-46.59
///     NATIVE captor st=0x1e1 f=42 | victim st=0x178 off=-39.46
///     NATIVE captor st=0x1e1 f=63 | victim st=0x179 off=-44.50
///
/// Twenty and twenty-one captor animation frames, with the victim moving five units in all of
/// them: the stretch and the hold. This is the "missing pieces of animation for the bounce against
/// the rope" from the playtest, and holding them for one frame is why "the timing needed to get the
/// ideal lariat is way too fast".
const ROPE_HOLD_FRAMES: i32 = 20;

/// THE TWO LEGS OF THE FLIGHT, TIMED BY THE GAME'S OWN FRAME COUNTS INSTEAD OF BY A DISTANCE.
///
/// This replaces a watcher that put the victim on the ropes the frame their captor-relative x
/// crossed `special_s_lariat_offset_x1` (-24). The native measurement says that is the wrong
/// mechanism and the wrong place:
///
///     f1   victim 0x176 CATCHED      off=+14.41   rope flags 00
///     f36  victim 0x177 THROWN       off=-7.09    rope flags 01   <- the ACMD's frame-37 flag
///     f44  victim 0x178 ATTACH_ROPE  off=-41.55   rope flags 01
///     f64  victim 0x179 RETURN       off=-46.59   rope flags 01
///     f73  follow-up                 off=+3.09
///
/// The victim is already BEHIND him at -7.09 when the throw fires -- the swing happens during
/// CATCHED -- and the rope catches them at -41.55, seventeen units past the -24 line. So -24 was
/// never the rope; it is a tripwire the victim crosses on the way out and again on the way back,
/// which is what the three area params are for and why they are all `node = top` with
/// `x1 == x2`.
///
/// THE LEG LENGTHS ARE PARAMS, AND THEY MATCH THE MEASUREMENT EXACTLY:
///
///     special_s_through_target_motion_frame_fix        = 8   THROWN, measured f36 -> f44
///     special_s_through_target_return_motion_frame_fix = 8   RETURN, measured f64 -> ~f73
///
/// So the outbound leg ends on a frame count, not at a place, and the distance it covers is
/// whatever `special_s_through_target_speed` = 11.2 and the engine's braking produce -- which is
/// the game deciding it rather than this port choosing it.
///
/// AND THE AREA FLAGS ARE NEVER RAISED NATIVELY. `area L=0 Lend=0 F=0` on every frame of all
/// three whips, while Incineroar still reached LARIAT and SHOULDER. Whatever raises them for him
/// is the rope article's business. This port raises them from the geometry instead -- see the
/// block above -- which now works for the first time, because the victim finally travels far
/// enough to cross the three lines on the way back.
unsafe fn whip_flight(fighter: &mut L2CFighterCommon, entry: usize, off: f32) {
    let boma = fighter.module_accessor;
    let id = state::swing_thrown_fighter(entry);
    let Some(vboma) = crate::victim::boma(id) else {
        return;
    };
    let vst = StatusModule::status_kind(vboma);
    let thrown = *FIGHTER_STATUS_KIND_SWING_GAOGAEN_THROWN;
    let attached = *FIGHTER_STATUS_KIND_SWING_GAOGAEN_ATTACH_ROPE;
    if vst != thrown && vst != attached {
        state::set_flight_frames(entry, 0);
        return;
    }
    let n = state::flight_frames(entry) + 1;
    state::set_flight_frames(entry, n);

    if vst == thrown {
        // THEY REACH THE ROPE AT A PLACE, AND THE FRAME COUNT IS ONLY THE BACKSTOP.
        //
        // The per-frame trace of our own whip says the engine is already doing this correctly:
        //
        //   f38 off=-18.29  f40 -29.49  f42 -40.69  f44 -51.89   <- 11.2/frame, the throw
        //   f46 -53.49  f48 -55.09  f50 -56.69  f52 -58.29       <- 1.6/frame, _speed_over_rope
        //
        // The speed drops from `special_s_through_target_speed` to `_speed_over_rope` at f44,
        // which is the victim's own status noticing it has passed `SPECIAL_S_FLOAT_ROPE_X`. The
        // rope is THERE. Waiting for `_motion_frame_fix` = 8 frames then let them creep another
        // seventeen units past it before attaching -- native ATTACH_ROPE begins at off=-41.55 and
        // ours was beginning at -58.29. That is "alolan whip throw distance still varies -- native
        // incineroar always throws 5 tiles, modified pikachu sometimes throws farther": the creep
        // is 1.6 a frame off a captor who is himself still sliding, so the overshoot is whatever
        // those two happen to add up to.
        //
        // So the trigger is the rope line we latched ourselves, compared in world x, which cannot
        // drift with the captor the way a captor-relative offset does. `_motion_frame_fix` stays
        // as the bound -- it is the game's own number for how long this leg may last, and without
        // it a victim who somehow never reaches the line would stay in THROWN forever.
        // ATTACH AT THE ROPE LINE. THE ENGINE WILL NOT SLOW THEM, SO WAITING COSTS A WHOLE STEP.
        //
        // The previous round waited until the victim had passed the line PLUS the give, on the
        // theory that `SPECIAL_S_FLOAT_ROPE_X` makes the engine drop them from `_speed` to
        // `_speed_over_rope` and that slow leg is the stretch. pt23 measured it and it does not:
        //
        //     f38 off=-18.29   f40 -29.49   f42 -40.69   f44 -51.89   vst=0x177
        //     f46..f64         -51.89 for every one of the twenty hold frames   vst=0x178
        //
        // Five full 11.20 steps where native takes four, and then dead still. Both hooks are
        // being read -- `rope_x` and `GENERATED_ROPE` are in the param log -- so the engine has
        // the number and does not act on it in time. At 11.20 a frame there is no landing
        // anywhere near the give: the step that crosses the line overshoots it by 11.20, which is
        // the whole of "still sometimes goes further than 5 tiles".
        //
        // Native's THROWN ends AT the line, off -40.69, and ours reaches exactly that on the
        // fourth step. So that is where this attaches now, and the give is paid out below.
        // TO THE NEAREST STEP, BECAUSE THE FLIGHT IS QUANTISED AND `<= 0.0` LOSES BY A HAIR.
        //
        // pt24 attached at off -51.89 on all 50 whips even though the line was right:
        //
        //     reached the ropes after 4f at off=-51.89 (rope_x=-51.58 their_x=-62.78)
        //
        // `their_x` is one full `_speed` step past `rope_x`, so the frame before it the victim
        // was at -51.58 -- rope_x to every digit printed. `(pos - rope_x) * lr <= 0.0` is an
        // exact-equality test between two floats reached by different arithmetic (the engine
        // integrating `start + 3 * 11.2`, and `captor_x + ROPE_OFFSET_X` latched at the throw),
        // and it came out a hair positive every single time. Consistently, because the same
        // arithmetic produces the same residue.
        //
        // The victim moves in whole `_speed` steps, so "has reached the line" cannot be resolved
        // finer than one step anyway: the honest test is which step lands NEAREST the line, and
        // half a step is the midpoint between the two candidates -- 0.0 away versus 11.2 away.
        // No tolerance constant, just the game's own speed.
        let lr = PostureModule::lr(boma);
        let speed = WorkModule::get_param_float(
            boma,
            hash40("param_special_s"),
            hash40("special_s_through_target_speed"),
        );
        let reached =
            (PostureModule::pos_x(vboma) - state::rope_x(entry)) * lr <= speed * 0.5;
        let bound = WorkModule::get_param_float(
            boma,
            hash40("param_special_s"),
            hash40("special_s_through_target_motion_frame_fix"),
        ) as i32;
        if reached || n >= bound.max(1) {
            crate::telemetry::sink::diag(format!(
                "[rat] WHIP p{entry} {id:#x} reached the ropes after {n}f at off={off:.2} \
                 (rope_x={:.2} their_x={:.2} by_place={reached} bound={bound}) -- attaching",
                state::rope_x(entry),
                PostureModule::pos_x(vboma)
            ));
            state::set_rope_spent(entry, 0.0);
            state::set_flight_frames(entry, 0);
            // The captor's animation frame at the moment of attaching, so the hold can be counted
            // in the same units it was measured in. See `ROPE_HOLD_FRAMES`.
            state::set_rope_hold_from(entry, MotionModule::frame(boma) as i32);
            crate::victim::request_status(vboma, attached, "whip rope caught them");
        }
        return;
    }

    // COUNTED IN THE CAPTOR'S ANIMATION FRAMES, WHICH IS THE UNIT IT WAS MEASURED IN.
    //
    // The native hold is captor animation frame 44 to 64. `game_specialsthrow` sets
    // `FT_MOTION_RATE(0.5)` from animation frame 20, so twenty ANIMATION frames there are forty
    // GAME frames -- and this counter was counting game frames, holding for half as long in
    // animation terms while the playtest read the whole bounce as too long anyway. Counting the
    // same thing the measurement counted removes the ambiguity entirely: if twenty animation
    // frames is wrong, the number is wrong, rather than the unit.
    // PAY OUT THE ROPE'S GIVE, ONE `_speed_over_rope` PER FRAME, BECAUSE THERE IS NO ROPE TO DO IT.
    //
    // Native ATTACH_ROPE does not hold the victim still: it slides them the last 7.84 out and
    // then settles, and the shape is identical across all 58 traced whips --
    //
    //     0x178  -41.55  -48.15  -48.32  -48.49  -48.63 ... -49.39
    //
    // -- a fast part that is four frames of `special_s_through_target_speed_over_rope` (1.6) and
    // a spring tail of 1.24. What drags them is the ROPE ARTICLE, which ADR 0004 does not ship,
    // so pt23 shows our victim frozen at one `off` for all twenty frames instead.
    //
    // THE MOVE IS FORCED INTO THIS STATUS BY THIS FILE, so driving it from this file is the same
    // ownership the rest of the whip already has -- `take_captive` calls `CaptureModule::capture`
    // by hand for exactly the same reason. The speed is the game's; only the total is measured,
    // and it stops dead once the measured give is spent rather than creeping the way the
    // pre-place version did.
    let spent = state::rope_spent(entry);
    if spent < ROPE_STRETCH_X {
        let over_rope = WorkModule::get_param_float(
            boma,
            hash40("param_special_s"),
            hash40("special_s_through_target_speed_over_rope"),
        );
        if over_rope > 0.0 {
            let step = over_rope.min(ROPE_STRETCH_X - spent);
            let lr = PostureModule::lr(boma);
            let p = Vector3f {
                x: PostureModule::pos_x(vboma) - step * lr,
                y: PostureModule::pos_y(vboma),
                z: 0.0,
            };
            PostureModule::set_pos(vboma, &p);
            state::set_rope_spent(entry, spent + step);
        }
    }

    let held = MotionModule::frame(boma) as i32 - state::rope_hold_from(entry);
    if held >= ROPE_HOLD_FRAMES {
        crate::telemetry::sink::diag(format!(
            "[rat] WHIP p{entry} rope releases {id:#x} after {held} animation frame(s) \
             ({n} game frames) at off={off:.2} -- rebounding"
        ));
        state::set_flight_frames(entry, 0);
        crate::victim::request_status(
            vboma,
            *FIGHTER_STATUS_KIND_SWING_GAOGAEN_RETURN,
            "whip rope rebound",
        );
    }
}
