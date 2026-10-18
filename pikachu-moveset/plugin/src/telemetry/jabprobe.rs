//! What the engine's own rapid-jab loop is deciding, frame by frame.
//!
//! WHY A PROBE RATHER THAN A FIFTH FIX. Four separate hypotheses about the rapid jab have now been
//! checked against the game's own data and every one came back IDENTICAL to Little Mac's:
//!
//!   * the ACMD. `game_attack100` is the same 93-line script, hit for hit -- one ATTACK at frame 0,
//!     `wait(1)`, `clear_all` + `on_flag(100_CONTINUE_CHECK)`, `wait_loop_clear`, then the same ten
//!     hits at frames 1, 3, 5 ... 19. Gate 3i compares every non-geometric argument.
//!   * the motion_list entries. `attack_100`, `attack_100_start` and `attack_100_end` dump
//!     byte-for-byte the same as his, `loop: true` and `no_stop_intp: true` included.
//!   * the animations. `c00attack100.nuanmb` final_frame_index 20.0, `c00attack100start` 2.0,
//!     `c00attackend` 39.0 -- all three the same as the source after the drop-in rescale.
//!   * the params. `attack100_type` true, `attack_100_enable_cnt` 4, `attack_combo_max` 3 and the
//!     two combo windows are patched into PIKACHU'S OWN ROW of fighter_param, which is the only
//!     mechanism the engine's native ComboModule can see.
//!
//! `Attack100SubL` and `Attack100SubR` were the fifth suspect and are a dead end: they appear in
//! Mac's script dump but name no motion_list entry in any of his eight costumes, so nothing invokes
//! them and not porting them is correct.
//!
//! WHAT IS LEFT IS THE STATUS, and the engine's decision is one function --
//! `L2CFighterCommon::sub_attack_100_uniq_check_button`, 0x710007f060, reached from
//! `status_Attack100_Main` through the default uniq hook at `0x710080ba30`. Transcribed:
//!
//!     if check_button_trigger(ATTACK) or check_button_release(ATTACK):  on_flag(ENABLE_COMBO_PRECEDE)
//!     if is_flag(100_LOOPED):
//!         if is_flag(100_CONTINUE_CHECK):
//!             off_flag(100_CONTINUE_CHECK)
//!             if is_flag(ENABLE_COMBO_PRECEDE):  off_flag(ENABLE_COMBO_PRECEDE)   # keep going
//!             elif check_button_off(ATTACK):     off_flag(100_CONTINUE)           # stop -> finisher
//!     else:
//!         if MotionModule::is_looped():  on_flag(100_LOOPED)
//!     if is_flag(100_CHECK_COMBO_NUM):
//!         off_flag(100_CHECK_COMBO_NUM)
//!         if 100_CONTINUE_COUNT > 0 and --100_CONTINUE_COUNT == 0:  off_flag(100_CONTINUE)
//!
//! READ THE `else` BRANCH TWICE. Nothing about continuing or stopping happens until `100_LOOPED` is
//! set, and `100_LOOPED` is set only once `MotionModule::is_looped()` has returned true -- so the
//! whole mechanism is downstream of the `attack_100` ANIMATION actually wrapping. The reported
//! symptom, "the initial rapid hits don't come out at all until the finishing blow", is what a loop
//! that never reports looping looks like from the outside: one hit from the top of the script, then
//! a script parked at `wait_loop_clear` until the status ends.
//!
//! So this prints `is_looped`, the five flags and the two counters on every frame of ATTACK_100 and
//! of the three jabs that lead into it. One play session answers it, and answers it about the real
//! build rather than about a reading of the disassembly.
//!
//! TURN IT OFF WHEN THE ANSWER IS IN. It writes a line per frame of an attack status, which is
//! cheap next to `SOURCE_TRACING` but is not free, and a log full of a solved question is a log
//! nobody reads.
use smash::{
    app::{lua_bind::*, BattleObjectModuleAccessor},
    lib::lua_const::*,
    lua2cpp::L2CFighterCommon,
};
use smashline::{Agent, Main};

use super::sink;

pub const ENABLED: bool = true;

/// THE COMPARISON IS WANTED AND IS NOT TAKEN, and gate 4j is why.
///
/// The obvious next measurement is one `on_line(Main, ..)` on Little Mac, reading his hitbox-live
/// count against ours: across one session our `attack_100` ran 1924 frames -- about 96 passes of a
/// 20-frame looping motion -- with an attack hitbox live on THIRTY-NINE of them. That is one hit
/// from the top of the script plus the ten it plays after `wait_loop_clear` resumes, then nothing
/// for ninety-odd passes. The ACMD runs about twice and stops while the motion goes on looping.
///
/// Everything around it is already cleared: the motion_list entry, the animation's frame count and
/// the ACMD are byte-identical to Mac's; the status machine is correct end to end; and the engine's
/// own STEP_LOOP does not restart the motion -- it only switches to `attack_100_end` when CONTINUE
/// drops. So the repeat is the MOTION's loop flag re-running the script, and ours stops doing it.
///
/// That probe was written and REMOVED. `tools/check-blastradius.sh` refuses any `Agent::new` for a
/// fighter this mod does not own, and its reason is exactly the symptom currently open: registering
/// a foreign agent makes smashline `load_fighter_module` and then spin in
/// `while !is_fighter_module_loaded { sleep(1ms) }` -- no timeout, no bail, once per foreign agent
/// AT FIGHT LOAD (vendor/sl_create_agent.rs:582 and :1079).
///
/// The gate also records, fairly, that forty playtests have armed source tracing and loaded fine,
/// so that edge has never actually cut. But there is an unexplained LOAD HANG open right now --
/// ARCropolis's inflate thread stopped at 107 of 455 on a second fight load while every other run
/// in the same capture completed -- and adding a known unbounded wait at fight load while
/// diagnosing a fight-load hang would make the next capture unreadable. Take the comparison once
/// the hang is understood.

/// The statuses worth printing: the rapid jab and the three jabs that reach it.
unsafe fn watched(status: i32) -> bool {
    status == *FIGHTER_STATUS_KIND_ATTACK_100 || status == *FIGHTER_STATUS_KIND_ATTACK
}

unsafe fn flag(boma: *mut BattleObjectModuleAccessor, id: i32) -> u8 {
    u8::from(WorkModule::is_flag(boma, id))
}

unsafe extern "C" fn jab_frame(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let status = StatusModule::status_kind(boma);
    if !watched(status) {
        return;
    }
    // Tagged, because the whole point is to read our lines against Little Mac's in one file.
    let who = if smash::app::utility::get_kind(&mut *boma) == *FIGHTER_KIND_PIKACHU {
        "rat"
    } else {
        "mac"
    };
    // `motion_kind` is the one field that distinguishes "parked in attack_100_start" from "looping
    // attack_100" from "already in attack_100_end", and the three are indistinguishable from the
    // status kind alone -- all three are FIGHTER_STATUS_KIND_ATTACK_100.
    sink::diag(format!(
        "[rat] JABPROBE {who} st={status} motion={:#x} frame={:.1}/{:.1} looped={} \
         LOOPED={} CONTINUE={} CONTINUE_CHECK={} PRECEDE={} CHECK_COMBO_NUM={} \
         100_COUNT={} CONTINUE_COUNT={} btn_off={} atk0={} atk1={} acmd_enter={} acmd_pass={} sleep={} blocks=[{}]",
        MotionModule::motion_kind(boma),
        MotionModule::frame(boma),
        MotionModule::end_frame(boma),
        u8::from(MotionModule::is_looped(boma)),
        flag(boma, *FIGHTER_STATUS_ATTACK_FLAG_100_LOOPED),
        flag(boma, *FIGHTER_STATUS_ATTACK_FLAG_100_CONTINUE),
        flag(boma, *FIGHTER_STATUS_ATTACK_FLAG_100_CONTINUE_CHECK),
        flag(boma, *FIGHTER_STATUS_ATTACK_FLAG_ENABLE_COMBO_PRECEDE),
        flag(boma, *FIGHTER_STATUS_ATTACK_FLAG_100_CHECK_COMBO_NUM),
        WorkModule::get_int(boma, *FIGHTER_STATUS_ATTACK_WORK_INT_100_COUNT),
        WorkModule::get_int(boma, *FIGHTER_STATUS_ATTACK_WORK_INT_100_CONTINUE_COUNT),
        u8::from(ControlModule::check_button_off(
            boma,
            *CONTROL_PAD_BUTTON_ATTACK
        )),
        // THE FIELD THE FIRST PASS WAS MISSING, and the one the whole question now turns on.
        //
        // The first capture settled the state machine and cleared it: `100_LOOPED` latches,
        // `100_CONTINUE_CHECK` is set by the ACMD and consumed by the engine once it has, holding
        // ATTACK keeps `PRECEDE` alive, releasing it sets `btn_off` and the very next check drops
        // `100_CONTINUE` and hands over to `attack_100_end`. That is the behaviour asked for, and
        // it is already happening -- 838 frames of `attack_100` against 236 of `attack_100_end`.
        //
        // So "the rapid hits don't come out" is not the loop failing to loop. It is either the
        // ACMD's ATTACK calls not producing hitboxes on the later passes, or them producing
        // hitboxes nobody is standing in. These two say which: a hitbox that is never live on a
        // pass after the first is a script that stopped running, and one that is live every pass
        // moves the question to damage and to the opponent.
        u8::from(AttackModule::is_attack(boma, 0, false)),
        u8::from(AttackModule::is_attack(boma, 1, false)),
        // THE TWO NUMBERS THE WHOLE QUESTION NOW TURNS ON, counted inside `game_attack100` itself
        // (tools/gen-moves.sh emits the calls; `ACMD_TRACE` names the script). Running totals, not
        // per-frame events, so they are read rather than taken -- watching them climb across one
        // hold is the measurement.
        //
        // Equal counts in the nineties: the script runs every loop and the hits are being lost
        // further down. An enter count in the nineties with a pass count of one: `wait_loop_clear`
        // is the wall, which would make it the first call in the script that never comes back.
        // Both at one or two: the VM is not re-entering the script at all, and the motion's loop
        // flag is not doing what every other fighter's rapid jab depends on.
        super::probe::acmd_peek(0).0,
        super::probe::acmd_peek(0).1,
        // Whether the VM considers the script finished. If `src/rapidjab`'s wake is the half that
        // matters this reads 1 on the wrap frames and the enter count climbs with it.
        u8::from(MotionAnimcmdModule::is_sleep(boma)),
        // THE ELEVEN HIT BLOCKS OF `game_attack100`, in order. Block 0 is the frame-0 ATTACK and
        // block 1 its clear; 2..21 are the ten ATTACK/clear pairs at frames 1, 3, 5 ... 19.
        // [N,N,0,0,...] means the script's frame base is wrong after the park and every later
        // `frame(N)` is in the past, so `is_excute` refuses. [N,N,N,...] all equal means the
        // ATTACKs are happening and the hitboxes are the problem.
        super::probe::acmd_blocks(12),
    ));
}

pub fn install() {
    if !ENABLED {
        return;
    }
    sink::diag(
        "[rat] JABPROBE armed on ATTACK and ATTACK_100 -- one line per frame of either, reporting \
         what sub_attack_100_uniq_check_button has to work with"
            .into(),
    );
    crate::charaslot::agent("pikachu")
        .on_line(Main, jab_frame)
        .install();
}
