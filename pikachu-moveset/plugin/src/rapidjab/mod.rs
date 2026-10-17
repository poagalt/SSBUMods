//! Keeps the rapid jab's ACMD running for as long as the motion loops.
//!
//! WHAT IS WRONG, MEASURED RATHER THAN REASONED. `tools/gen-moves.sh` emits two counters into
//! `game_attack100` itself (`ACMD_TRACE`), and one hold of the button reads:
//!
//!     acmd_enter=1  acmd_pass=0   ... frames 0 through 19 of the motion
//!     acmd_enter=1  acmd_pass=1   ... and 1/1 for the whole rest of the hold
//!
//! The VM enters the script ONCE. It parks at `wait_loop_clear` -- which the engine implements as
//! a yield (`sv_animcmd::wait_loop_clear`, main 0x22b5870: it suspends unless a loop predicate
//! holds) -- resumes at the first wrap, plays its ten hits across frames 1..19, returns, and is
//! never entered again. Meanwhile the motion goes on looping about twenty-two more times. An attack
//! hitbox is live on 75 frames of 446, and all of it is that single pass.
//!
//! EVERYTHING ELSE WAS ELIMINATED FIRST, which is why this is the only thing left:
//!
//!   * the ACMD is byte-identical to Little Mac's, and all 38 fighters with a rapid jab have
//!     exactly one `wait_loop_clear` near the top of `game_attack100` -- our placement is the
//!     universal shape, not a mistake
//!   * the `attack_100` motion_list entry matches Mac's on every field `mlshow` reads, which is
//!     every field the struct has: game_script, scripts, animations, blend, faf/xlu/no_stop_intp
//!     and the whole flag bitfield including `loop: true`
//!   * the animation's `final_frame_index` is 20.0 on both
//!   * the motion really does loop -- the measured frame sequence is 0,1,2..19,0,1,2..19
//!   * `is_looped` fires, `100_LOOPED` latches, and the status machine is correct end to end:
//!     holding ATTACK keeps `ENABLE_COMBO_PRECEDE` alive, releasing it drops `100_CONTINUE` and
//!     the engine hands over to `attack_100_end`
//!   * the engine's own STEP_LOOP (`status_Attack100_Main_uniq_func`) does not restart the motion
//!     and does not need to -- on a vanilla fighter the repeat comes from the motion's loop flag
//!     re-running the script, which is exactly what is not happening here
//!
//! So the gap is between the looping motion and the script, and that is a seam this mod owns:
//! these scripts are installed by smashline rather than by the fighter's own
//! `create_agent_fighter_animcmd_game`, and `attack_100` is the only LOOPING motion in the whole
//! moveset -- no other ported move would ever have shown this.
//!
//! WHAT THIS DOES, AND WHY THESE TWO CALLS. On the frame the motion wraps, ask the engine to run
//! the script again:
//!
//!   `MotionAnimcmdModule::is_sleep` / `set_sleep(false)` -- a script whose function has returned
//!   is a slept script, and an unslept one is the VM's own notion of "there is more to run". This
//!   is the gentle half: it changes no frame and no motion.
//!
//!   `MotionModule::set_frame_sync_anim_cmd(0.0)` -- "set the frame AND sync the animcmd", the
//!   engine's own way of putting a motion and its script back to the same point. The motion has
//!   just wrapped to 0 of its own accord, so as an animation change this is a no-op; what it is
//!   for is the script half.
//!
//! IT REPORTS WHETHER IT WORKED, which is the point of doing it this way round. The counters stay
//! in, so the next capture answers it outright: `acmd_enter` climbing to roughly twenty-two per
//! hold with the hitbox-live count rising to match means this is right. `acmd_enter` still stuck
//! at 1 means neither call is the seam and the `sleep=` field says which half was even plausible.
//! No part of this is a guess that cannot be read back.
//!
//! SCOPED AS NARROWLY AS IT CAN BE: one status, one motion, and only on the frame the motion's own
//! frame counter goes backwards. Off through `ENABLED` without unpicking the module.
use smash::{
    app::{lua_bind::*, BattleObjectModuleAccessor},
    lib::lua_const::*,
    lua2cpp::L2CFighterCommon,
    phx::Hash40,
};
use smashline::Main;

pub const ENABLED: bool = true;

/// Last motion frame seen, per entry, so a wrap can be told from ordinary advance.
static mut LAST_FRAME: [f32; 8] = [-1.0; 8];

unsafe fn entry_of(boma: *mut BattleObjectModuleAccessor) -> usize {
    let e = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if e < 8 {
        e
    } else {
        0
    }
}

unsafe extern "C" fn rapid_jab_frame(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    if StatusModule::status_kind(boma) != *FIGHTER_STATUS_KIND_ATTACK_100 {
        return;
    }
    // Only the LOOP motion. `attack_100_start` and `attack_100_end` each run once by design, and
    // re-triggering the finisher would be a second knockback hit out of one input.
    if MotionModule::motion_kind(boma) != Hash40::new("attack_100").hash {
        return;
    }
    let entry = entry_of(boma);
    let now = MotionModule::frame(boma);
    let prev = LAST_FRAME[entry];
    LAST_FRAME[entry] = now;
    // A wrap is the frame counter going BACKWARDS. `is_looped` is not used for this: it was
    // measured firing once per episode rather than once per loop, so it answers a different
    // question than the one here.
    if prev < 0.0 || now >= prev {
        return;
    }
    if MotionAnimcmdModule::is_sleep(boma) {
        MotionAnimcmdModule::set_sleep(boma, false);
    }
    // ARGUMENTS TAKEN FROM THE ENGINE'S OWN USE OF IT, not guessed. `sub_wait_motion_mtrans`
    // (common) is the canonical resync of a LOOPING motion's animcmd -- the same shape of problem
    // -- and it calls `set_frame_sync_anim_cmd(boma, 0.0, true, false, false)`. Ghidra renders that
    // as four arguments because the float rides a float register and does not appear in the
    // integer list; the frame it pushes just above the call is 0.
    MotionModule::set_frame_sync_anim_cmd(boma, 0.0, true, false, false);
}

pub fn install() {
    if !ENABLED {
        return;
    }
    crate::telemetry::sink::diag(
        "[rat] RAPIDJAB re-triggering the attack_100 script on every motion wrap; watch \
         acmd_enter in JABPROBE -- about 22 per hold means it took, still 1 means it did not"
            .into(),
    );
    crate::charaslot::agent("pikachu")
        .on_line(Main, rapid_jab_frame)
        .install();
}
