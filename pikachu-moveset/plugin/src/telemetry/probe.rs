// Instrumentation the hand-written status modules call directly.
//
// WHY THIS EXISTS, AND WHY IT IS WORTH THE LINES. Every gameplay fact about this mod has come back
// through a human describing what they saw, and each round the description has been accurate and
// the CAUSE has been somewhere nobody was looking. The up-B was reported invisible three times;
// the status logic was right all along and the entry was naming Pikachu's own expression script.
// The cargo freeze took a crash report to place. Neither was visible in the telemetry, because
// the telemetry logged what STATUS the fighter was in and nothing about why our code chose it.
//
// So: every decision these modules make gets a line, with the inputs that drove it. It is verbose
// on purpose. A playtest is expensive -- it needs a person, a console and a controller -- and the
// cost of a log line is nothing next to the cost of needing another one.
//
// THREE THINGS THIS FIXES ABOUT THE PREVIOUS ROUND, each of which cost real time:
//
//   1. The buffer lost the crash. Telemetry flushed every 120 frames, and a hard freeze threw away
//      up to two seconds -- exactly the frames between entering the throw and the crash. Anything
//      logged through `event` here flushes immediately.
//   2. Nothing said what a status number meant. The log read `status 3 -> 477` and every reading
//      needed a lookup table that lived in someone's head. `status_name` covers the moveset's own
//      kinds and the common ones the moveset touches.
//   3. A crash inside `MotionModule::change_motion` was unattributable -- the plugin's own frames
//      do not appear in an Atmosphere backtrace, so "which of our change_motion calls was it"
//      could not be answered. `change_motion` here logs the target BEFORE calling, so the last
//      line in the file names it.
use smash::{
    app::{lua_bind::*, BattleObjectModuleAccessor},
    lib::lua_const::*,
    phx::Hash40,
};

use super::sink;

/// Master switch. Verbose logging is the point right now; when the moveset is stable this drops
/// to false and the per-frame probes cost one branch.
pub const VERBOSE: bool = true;

/// The name for a status kind, or "" when nothing names it.
///
/// THE GAME'S OWN TABLE FIRST, THEN THIS MOVESET'S OWN KINDS. `statusnames` is generated from
/// `vendor/ssbu-status/lua_dict.txt` against the crate's runtime statics -- all 508 common kinds
/// -- so no number in a log is unnamed and none of them is a number somebody remembered.
///
/// THE HAND-WRITTEN TABLE THIS REPLACES WAS WRONG, and it cost four rounds. It had
/// `56 => "CATCH_PULL", 57 => "CATCH_WAIT"` as literals, so a native Donkey Kong reaching the
/// carry from 0x39 and our Pikachu reaching THROW from 0x3c printed as the same name and looked
/// like one story. Only the moveset's OWN kinds are listed by hand below, because they are ours
/// and the game's table has never heard of them.
pub fn status_name(k: i32) -> &'static str {
    let own = super::statusnames::of_own(k);
    if !own.is_empty() {
        return own;
    }
    super::statusnames::of(k)
}

/// Format a status as `name(0x1f2)`, or just the number when unnamed.
pub fn st(k: i32) -> String {
    let n = status_name(k);
    if n.is_empty() {
        format!("{k}(={k:#x})")
    } else {
        format!("{n}({k:#x})")
    }
}

/// One immediately-flushed line. For decisions and transitions -- things that happen a handful of
/// times per move, where losing them to a crash is the whole problem.
pub fn event(line: String) {
    if VERBOSE {
        sink::diag(line);
    }
}

/// One buffered line. For per-frame probes, where flushing would be a performance bug and the
/// preceding `event` already anchors them in time.
pub fn frame(line: String) {
    if VERBOSE {
        sink::log(line);
    }
}

/// Where the fighter is and what it is doing, in one field set. Appended to most probe lines
/// because "on which frame of what, at what height, moving how fast" is the question that follows
/// every report.
pub unsafe fn ctx(boma: *mut BattleObjectModuleAccessor) -> String {
    format!(
        "st={} mot={:#012x} f={:.1}/{:.0} pos=({:.1},{:.1}) vel=({:.2},{:.2}) sit={}",
        st(StatusModule::status_kind(boma)),
        MotionModule::motion_kind(boma),
        MotionModule::frame(boma),
        MotionModule::end_frame(boma),
        PostureModule::pos_x(boma),
        PostureModule::pos_y(boma),
        KineticModule::get_sum_speed_x(boma, *KINETIC_ENERGY_RESERVE_ATTRIBUTE_MAIN),
        KineticModule::get_sum_speed_y(boma, *KINETIC_ENERGY_RESERVE_ATTRIBUTE_MAIN),
        StatusModule::situation_kind(boma),
    )
}

/// Change motion, but say so first and refuse if the target does not exist.
///
/// THE CARGO FREEZE WENT THROUGH `MotionModule::change_motion` WITH A NULL DEREFERENCE, and the
/// Atmosphere backtrace could name Training Modpack's hook and the engine's own frames but not
/// ours -- the plugin's Rust frames are not in the chain. So "which of our change_motion calls
/// was it" was unanswerable from the report and had to be inferred from the surrounding status
/// numbers.
///
/// Logging the target BEFORE the call makes the last line in the file the answer. The
/// `end_frame_from_hash` guard is cheap insurance on top: a motion the game cannot resolve is one
/// the engine may hand a null entry, and refusing is always better than freezing.
/// Unused since the motion-swap carry was deleted. Kept because it is the only helper that
/// REFUSES a motion change the game would have rejected and says so, which is worth having the
/// next time a move needs to set one outside a status main.
#[allow(dead_code)]
pub unsafe fn change_motion(boma: *mut BattleObjectModuleAccessor, name: &str, tag: &str) -> bool {
    let h = Hash40::new(name);
    let end = MotionModule::end_frame_from_hash(boma, h);
    if !MotionModule::is_anim_resource(boma, h) || end < 1.0 {
        event(format!(
            "[rat] PROBE {tag} REFUSED change_motion to '{name}' -- unresolved (end={end:.0}); {}",
            ctx(boma)
        ));
        return false;
    }
    event(format!("[rat] PROBE {tag} change_motion -> '{name}' (end={end:.0}); {}", ctx(boma)));
    MotionModule::change_motion(boma, h, 0.0, 1.0, false, 0.0, false, false);
    true
}

/// WHY AN ORDINARY GRAB DID NOT CONVERT, MEASURED AT THE MOMENT IT STARTS.
///
/// A BLOCKED GRAB IS NOT A WHIFF, and telling them apart was the whole difficulty. The digest is
/// unambiguous once you have both shapes side by side:
///
///     CATCH 7f  -> CATCH_PULL   connected -- Pikachu's catch frames are 6 and 7
///     CATCH 51f -> WAIT         whiffed   -- the whole fifty-frame animation ran
///     CATCH 8f  -> WAIT         connected, then failed to convert
///
/// Fourteen of the third kind are in the logs, and for three rounds they were read as whiffs and
/// answered by looking at Pikachu's reach. They are not about reach: the collision landed on the
/// frame it always lands on, and the engine then declined to start the grab. What can decline it
/// is the target -- a fighter the engine still believes is captured, or one left with
/// `ignore_catching`, cannot be caught by anybody.
///
/// So this prints the answer instead of inferring it. Every live fighter, every flag the engine
/// consults, on the frame Pikachu enters CATCH. Cheap: once per grab attempt.
pub unsafe fn catch_start(entry: usize, me: *mut BattleObjectModuleAccessor) {
    use smash::app::lua_bind::{CaptureModule, CatchModule};
    let mut line = format!(
        "[rat] CATCH p{entry} starting: mine is_catch={} ignore_catching={} is_capture={}",
        CatchModule::is_catch(me),
        CaptureModule::is_ignore_catching(me),
        CaptureModule::is_capture(me),
    );
    for (id, them) in crate::victim::others(crate::victim::id_of_entry(entry as u32)) {
        line.push_str(&format!(
            " | {id:#x} st={} is_capture={} ignore_catching={} captured_by={:#x} clatter={}",
            st(smash::app::lua_bind::StatusModule::status_kind(them)),
            CaptureModule::is_capture(them),
            CaptureModule::is_ignore_catching(them),
            CaptureModule::catch_object_id(them) as u32,
            smash::app::lua_bind::ControlModule::get_clatter_time(them, 0),
        ));
    }
    event(line);
}

/// The same, but told whether the fighter is OURS.
///
/// `status_name` consults `statusnames::of_own` first, and that table is 0x1F0..0x206 -- kinds
/// that exist only on this Pikachu. Applied to any other fighter it is simply wrong, because every
/// fighter's own ids start where the common ones end: Donkey Kong's 0x1F0 is SHOULDER_JUMP, his
/// 0x1F1 is SHOULDER_FALL and his 0x1F7 is THROW_F_HI.
///
/// THAT COST A WRONG DIAGNOSIS. Hunting the crash in native DK's cargo throw, pt36 read
/// `DONKEY0 status ... -> RAT_SHOULDER_START(0x1f7)` and the obvious conclusion was that DK had
/// been pushed into one of Pikachu's statuses. He had not -- 0x1F7 is his own THROW_F_HI, and the
/// real bug (his forward-throw slot repointed at it) was worse and in a different place. A log
/// that names a foreign fighter's statuses out of our table is a log that argues for the wrong
/// fix.
///
/// So a foreign fighter gets the COMMON table only, which is valid for everyone, and anything past
/// it prints as a bare number -- which is honest rather than confident.
pub fn st_kind(k: i32, ours: bool) -> String {
    if ours {
        return st(k);
    }
    let n = super::statusnames::of(k);
    if n.is_empty() {
        format!("{k}(={k:#x})")
    } else {
        format!("{n}({k:#x})")
    }
}

// ================================================================================================
// THE BACK THROW, WHICH IS THE ONE PORTED THROW THAT DOES NOT WORK
// ================================================================================================
//
// Playtest: "trying to back throw makes pikachu let go of the opponent and immediately run
// backwards, not actually throw."
//
// EVERYTHING THAT CAN BE SETTLED WITHOUT RUNNING THE GAME HAS BEEN, AND ALL OF IT IS RIGHT. That
// list matters as much as the probe, because each item is a fix somebody would otherwise try:
//
//   * `game_throwb` is Ness's, copied verbatim by tools/gen-moves.sh, and it is STRUCTURALLY
//     IDENTICAL to Pikachu's own working back throw: two `ATTACK_ABS` on frame 0 (one THROW, one
//     CATCH), `CHECK_FINISH_CAMERA`, then one `ATK_HIT_ABS` on the `throw` node reading the three
//     `FIGHTER_STATUS_THROW_WORK_INT_TARGET_*` work ids. Pikachu's fires on frame 26, Ness's on 27.
//   * the animation IS Ness's. `e01throwb.nuanmb` is dropped in, 68 frames, and it still carries a
//     `Throw` node track after the drop-in strips the 36 bones Pikachu does not have.
//   * the motion entry carries Ness's `flags`, `blend` and FAF exactly -- `turn` set, `move`
//     clear, faf 53. Vanilla Pikachu's own `throw_b` ALSO has `turn` (and `move`, which Ness does
//     not need), so turning is not the difference, and `move` going away is correct: Ness's back
//     throw has no root motion to apply.
//   * `Throw` is in the shipped skeleton, so the node `ATK_HIT_ABS` names resolves.
//   * THE CARGO CARRY CANNOT BE STEALING IT. `retarget_forward_throw` writes `globalTable[0x45]`
//     and nothing else, and the common `CatchCont` sends a BACK throw to `globalTable[0x46]` --
//     read out of vendor/decomp, not assumed. `pikachu_throw_main` delegates any throw whose
//     `CATCH_WAIT_WORK_INT_MOTION_KIND` is not `throw_f`, and `status_Throw_Sub` proves that work
//     id really is the chosen throw's motion hash.
//   * NESS'S `Throw` STATUS OVERRIDE, WHICH WAS THE STANDING SUSPECT, IS COSMETIC. It was the one
//     structural difference left: Ness is the only donor here who replaces the common Throw init,
//     and his installs a per-frame helper into `globalTable[0x15]` -- the same slot Donkey Kong
//     uses for his own per-frame helpers. Disassembled (ness `0x7100006190`), that helper is:
//
//         if arg and AttackModule::is_hit_abs(boma) then
//             ModelModule::set_joint_translate(boma, hash40("throw"), {0.0, 2.0, 0.0}, false, true)
//         end
//
//     One joint lift, two units up, only AFTER the throw has already connected. It cannot be what
//     stops the throw connecting, and it is deliberately NOT ported: `set_joint_translate` has no
//     matching clear in Ness's tree, so a faithful copy risks leaving the node the victim rides
//     permanently displaced, and ADR 0005 does not allow shipping that to fix a bug it cannot
//     cause.
//
// So the cause is something only the running game can show, and this is the probe for it. Two
// lines per throw attempt, which is why it can stay on:
//
//   in   which motion the engine chose, the work ids the ACMD will read at frame 27, and whether
//        the grab is still standing on the way IN. If `MOTION_KIND` is not `throw_b`, the wrong
//        throw was selected and nothing after it matters. If `TARGET_OBJECT` is already invalid,
//        the throw was never going to connect and the release happened BEFORE this status.
//   out  the frame reached against the animation's length, and whether `AttackModule::is_hit_abs`
//        ever became true. Ness's own status helper reads exactly that flag to decide the throw
//        landed, so it is the engine's own answer to "did the throw connect", not ours.
//
// Between them those two lines separate the three candidate stories -- wrong motion selected, the
// grab already gone on entry, or the status ending before frame 27 -- and no two of them want the
// same fix.
static mut THROW_IN: [bool; 8] = [false; 8];
static mut THROW_HIT: [bool; 8] = [false; 8];
static mut THROW_FRAME: [f32; 8] = [0.0; 8];

/// The four throw motions by hash, so the line names the throw instead of printing a bare number
/// the reader has to look up. `st()` exists for exactly this reason on the status side.
fn throw_motion(h: u64) -> &'static str {
    for name in ["throw_f", "throw_b", "throw_hi", "throw_lw", "throw_kirby"] {
        if Hash40::new(name).hash == h {
            return name;
        }
    }
    "?"
}

/// Call once per frame with the fighter's current status. Does nothing outside `THROW`.
pub unsafe fn throw_frame(
    entry: usize,
    boma: *mut BattleObjectModuleAccessor,
    status: i32,
    frame: f32,
    end: f32,
) {
    if entry >= 8 {
        return;
    }
    let in_throw = status == *FIGHTER_STATUS_KIND_THROW;

    if in_throw && !THROW_IN[entry] {
        THROW_IN[entry] = true;
        THROW_HIT[entry] = false;
        THROW_FRAME[entry] = 0.0;
        let chosen = WorkModule::get_int64(boma, *FIGHTER_STATUS_CATCH_WAIT_WORK_INT_MOTION_KIND);
        event(format!(
            "[rat] THROW p{entry} in: chose {}({chosen:#012x}) rate={:.3} | target={:#x} \
             group={} no={} | is_catch={} capture_object={:#x} linked={}",
            throw_motion(chosen),
            WorkModule::get_float(boma, *FIGHTER_STATUS_THROW_WORK_FLOAT_MOTION_RATE),
            WorkModule::get_int64(boma, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT),
            WorkModule::get_int64(boma, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_GROUP),
            WorkModule::get_int64(boma, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_NO),
            CatchModule::is_catch(boma),
            CatchModule::capture_object_id(boma, false) as u32,
            LinkModule::is_linked(boma, *LINK_NO_CAPTURE),
        ));
    }

    if in_throw {
        THROW_FRAME[entry] = frame;
        if AttackModule::is_hit_abs(boma) {
            THROW_HIT[entry] = true;
        }
        return;
    }

    if THROW_IN[entry] {
        THROW_IN[entry] = false;
        event(format!(
            "[rat] THROW p{entry} out on frame {:.0} of {end:.0}: is_hit_abs ever true = {} -- \
             the ATK_HIT_ABS is on frame 27, so reaching fewer than that means the status ended \
             first and no fix to the script can help",
            THROW_FRAME[entry], THROW_HIT[entry],
        ));
    }
}

// ---- ACMD execution counters -------------------------------------------------------------------
//
// DOES THE SCRIPT RUN AT ALL, AND HOW OFTEN. Everything else about the rapid jab is now measured
// and faithful -- the motion loops cleanly every 20 frames about 96 times a hold, the motion_list
// entry and animation frame count are byte-identical to Little Mac's, the ACMD is byte-identical,
// and the engine's own STEP_LOOP neither restarts the motion nor needs to. What is NOT measured is
// the one thing in the middle: how many times the ACMD VM actually enters `game_attack100`, and how
// many of those get past `wait_loop_clear`, which the engine implements as a YIELD
// (`sv_animcmd::wait_loop_clear`, main 0x22b5870: it suspends unless a loop predicate holds).
//
// The hitbox field says an attack box is live on 39 frames of 1924, which is about one hit plus the
// ten the script plays after the park resumes -- consistent with the script running ONCE. But
// "consistent with" is not "measured", and the two candidates it cannot separate are:
//
//   * the VM never re-enters the script after it returns, so the hits are gone for good; or
//   * it re-enters every loop and the script parks forever at `wait_loop_clear` on the second
//     entry, because that call marks itself done (`*(state + 9) = 1`) and nothing resets it.
//
// The counts tell those apart in one hold: equal enter/pass counts in the nineties means the script
// is running and the hits are being lost somewhere further down; an enter count in the nineties
// with a pass count of one means the park is the wall; both at one or two means the VM is not
// coming back.
//
// COUNTED, NOT LOGGED PER CALL, because these sit inside a script that runs every frame of a
// twenty-frame motion: a line each would be thousands. The tally is printed when the fighter leaves
// the status, which `jabprobe` already sees.
static mut ACMD_ENTER: [u32; 4] = [0; 4];
static mut ACMD_PASS: [u32; 4] = [0; 4];

/// Slot 0 is the rapid jab's loop script; the rest are spare so a second question needs no new
/// array. Out-of-range is ignored rather than panicking -- this is instrumentation and must never
/// be the thing that takes the game down.
pub unsafe fn acmd_enter(slot: usize) {
    if slot < 4 {
        ACMD_ENTER[slot] = ACMD_ENTER[slot].saturating_add(1);
    }
}

pub unsafe fn acmd_pass(slot: usize) {
    if slot < 4 {
        ACMD_PASS[slot] = ACMD_PASS[slot].saturating_add(1);
    }
}

/// Reads the tally without disturbing it, for printing alongside a line already being written.
pub unsafe fn acmd_peek(slot: usize) -> (u32, u32) {
    if slot >= 4 {
        return (0, 0);
    }
    (ACMD_ENTER[slot], ACMD_PASS[slot])
}

/// Reads the tally and zeroes it, for printing once per episode.
pub unsafe fn acmd_take(slot: usize) -> (u32, u32) {
    if slot >= 4 {
        return (0, 0);
    }
    let out = (ACMD_ENTER[slot], ACMD_PASS[slot]);
    ACMD_ENTER[slot] = 0;
    ACMD_PASS[slot] = 0;
    out
}
