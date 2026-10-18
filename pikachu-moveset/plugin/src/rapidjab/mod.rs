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
//!   `MotionModule::change_motion` back to `attack_100` -- a FRESH motion instance, which is the
//!   part that matters. `set_frame_sync_anim_cmd` was tried first and restarts the script while
//!   leaving the instance alone, and the instance is what `wait_loop_clear` waits on: the jab hit
//!   correctly for about a second and then stopped hitting at all while Pikachu kept jabbing,
//!   because the restarted script reached its park and the loop signal had already been spent.
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

/// The first block AFTER `wait_loop_clear` -- the ATTACK at script frame 1.
///
/// `game_attack100`'s blocks are numbered by the generator in source order: 0 is the frame-0
/// ATTACK, 1 its clear, and 2 is the first hit of the ten that follow the park. So "block 2 has
/// fired" is exactly "the resumed script got going", which is the thing this module must wait for
/// before it is allowed to restart anything.
const FIRST_TAIL_BLOCK: usize = 2;

/// Block 2's count at this entry's last restart, so the test is "since I last restarted".
static mut TAIL_AT_RESTART: [u32; 8] = [0; 8];

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
    // Reset alongside the counters: `jabprobe` zeroes them when ATTACK_100 is entered, so a
    // marker left over from the previous rapid jab would make the first restart of this one fire
    // on a stale comparison.
    if crate::telemetry::probe::acmd_block_count(0) == 0 {
        TAIL_AT_RESTART[entry] = 0;
    }
    let now = MotionModule::frame(boma);
    let prev = LAST_FRAME[entry];
    LAST_FRAME[entry] = now;
    // A wrap is the frame counter going BACKWARDS. `is_looped` is not used for this: it was
    // measured firing once per episode rather than once per loop, so it answers a different
    // question than the one here.
    if prev < 0.0 || now >= prev {
        return;
    }
    // ONLY WHEN THE SCRIPT HAS FINISHED, AND THE FIRST VERSION OF THIS DID NOT CHECK.
    //
    // Restarting on EVERY wrap made it worse in an instructive way. The counters read `e1 p0`
    // through the first cycle, then at the wrap BOTH climbed -- `e2 p1` -- meaning the parked
    // script resumed and the restart immediately pre-empted it. The ten hits the script plays
    // after `wait_loop_clear` never got to run; only the frame-0 hit of each fresh start did. One
    // hit per twenty-frame loop, which the playtest measured from the other side as "only around
    // every half second does one small jab hit".
    //
    // `enter` and `pass` say which state the script is in, because `wait_loop_clear` sits between
    // them: parked is `enter == pass + 1`, and a script that has resumed and run to its end is
    // `enter == pass`. So restart only on equality, and the natural rhythm comes back -- one hit
    // on the cycle that starts the script, ten on the cycle its park resumes into, repeating.
    //
    // `is_sleep` was the first candidate for this test and it is not one: measured 0 on every
    // frame of every cycle, parked or not. It stays in the probe line as a negative result.
    // WAIT FOR THE TAIL TO HAVE RUN, AND `enter == pass` DOES NOT MEAN THAT. That was the gate
    // here and it was the bug, not the fix for it. The measured cycle is:
    //
    //     f0  block0 (the ATTACK)   enter=2 pass=1
    //     f1  block1 (its clear), then the script parks at wait_loop_clear
    //     f2..f19  nothing
    //     f0  park resumes -> pass becomes 2 -- and THIS callback then runs in the SAME frame,
    //         reads enter==pass, restarts the motion, and the resumed script dies before block 2
    //
    // So the gate fired every single cycle and destroyed the ten tail hits every single cycle,
    // which is "only around every half second does one small jab hit". `enter == pass` cannot
    // express "the script has finished" because the resume and this check land on one frame.
    //
    // Block 2 can. It is the first hit after the park, so it is non-zero only once the resumed
    // script has actually started running its tail. Restarting only after it has advanced since
    // the last restart gives the script a whole cycle to play all ten, which is the rhythm it is
    // written for.
    let tail = crate::telemetry::probe::acmd_block_count(FIRST_TAIL_BLOCK);
    if tail <= TAIL_AT_RESTART[entry] {
        return;
    }
    TAIL_AT_RESTART[entry] = tail;
    // A FULL `change_motion`, NOT `set_frame_sync_anim_cmd`, AND THE DIFFERENCE IS THE LOOP STATE.
    //
    // The frame-sync version restarted the script and left the motion instance alone -- and the
    // motion instance is what `wait_loop_clear` waits on. Measured from the other side: the jab
    // worked for about a second and then stopped hitting at all while Pikachu kept jabbing. That
    // is the restarted script reaching its park and the park never resuming, because the loop
    // signal it waits for had already been spent on the first wrap of a motion that was never
    // replaced.
    //
    // `change_motion` to the same motion gives a fresh instance, so the signal is available again
    // and the park resumes on the NEXT wrap, which is the rhythm the script is written for: one
    // hit on the cycle that starts it, ten on the cycle its park resumes into. It is also what the
    // engine does in this very status -- `status_Attack100_Main_uniq_func` hands over to
    // `attack_100_end` with exactly this call -- and the motion has just wrapped to frame 0 of its
    // own accord, so as an animation change it is invisible.
    MotionModule::change_motion(
        boma,
        Hash40::new("attack_100"),
        0.0,
        1.0,
        false,
        0.0,
        false,
        false,
    );
}

pub fn install() {
    if !ENABLED {
        return;
    }
    crate::telemetry::sink::diag(
        "[rat] RAPIDJAB re-triggering the attack_100 motion when its script has run out; watch          acmd_enter and acmd_pass in JABPROBE -- both climbing together means the park is          resuming, acmd_pass stalling means it is not"
            .into(),
    );
    crate::charaslot::agent("pikachu")
        .on_line(Main, rapid_jab_frame)
        .install();
}
