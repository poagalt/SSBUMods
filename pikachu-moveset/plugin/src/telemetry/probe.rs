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

/// Names for the status kinds this moveset creates or routes through, so the log is readable
/// without a lookup table. Anything not here prints as a number, which is still better than the
/// previous state of everything printing as a number.
pub fn status_name(k: i32) -> &'static str {
    // THE VICTIM-SIDE FAMILY COMES FROM THE GAME'S OWN CONSTANTS, NOT FROM LITERALS, and that is
    // the whole reason this block exists separately from the match below.
    //
    // `HOLD p0 dropped the capture (...): their_capture=true their_st=0x44` is the single most
    // important line this project prints -- it is the state of the fighter we just let go -- and
    // `0x44` is unreadable. A comment in `crate::victim` asserts it is CAPTURE_CUT; nothing local
    // proves that, and asserting a status number from memory is exactly the kind of invented fact
    // this moveset is not allowed to have. Comparing against the LuaConst makes the log say the
    // name, so there is nothing left to remember or to get wrong.
    //
    // These are COMMON statuses, not any one fighter's, so naming them here is not the thing
    // `tools/check-status.sh` forbids. The SWING_GAOGAEN ids are the whip's victim statuses and
    // `specials::whip` already references them for the same reason.
    let victim: &[(i32, &'static str)] = &[
        (*FIGHTER_STATUS_KIND_CAPTURE_CUT, "CAPTURE_CUT"),
        (*FIGHTER_STATUS_KIND_CAPTURE_WAIT, "CAPTURE_WAIT"),
        (*FIGHTER_STATUS_KIND_CAPTURE_PULLED, "CAPTURE_PULLED"),
        (*FIGHTER_STATUS_KIND_THROWN, "THROWN"),
        (*FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED, "SWING_CATCHED"),
        (*FIGHTER_STATUS_KIND_SWING_GAOGAEN_THROWN, "SWING_THROWN"),
        (*FIGHTER_STATUS_KIND_SWING_GAOGAEN_ATTACH_ROPE, "SWING_ATTACH_ROPE"),
        (*FIGHTER_STATUS_KIND_SWING_GAOGAEN_RETURN, "SWING_RETURN"),
        (*FIGHTER_STATUS_KIND_SWING_GAOGAEN_LARIAT, "SWING_LARIAT"),
        (*FIGHTER_STATUS_KIND_SWING_GAOGAEN_SHOULDER, "SWING_SHOULDER"),
        (*FIGHTER_STATUS_KIND_SWING_GAOGAEN_FAILURE, "SWING_FAILURE"),
    ];
    for (id, name) in victim {
        if k == *id {
            return name;
        }
    }
    match k {
        // The moveset's own kinds, from src/specials/status.rs.
        0x1F0 => "RAT_SPECIAL_HI_CLOSE",
        0x1F1 => "RAT_SPECIAL_HI_OPEN",
        0x1F2 => "RAT_SPECIAL_S_SHOULDER",
        0x1F3 => "RAT_SPECIAL_S_LARIAT",
        0x1F4 => "RAT_SPECIAL_S_THROW",
        0x1F5 => "RAT_SPECIAL_S_END",
        0x1F6 => "RAT_SPECIAL_S_FAILURE",
        // Pikachu's own extras, which ../lockout bounces out of. Seeing one of these in the log
        // means something reached a retired move.
        0x1E1 => "pika_SPECIAL_HI_WARP",
        0x1E2 => "pika_SPECIAL_HI_END",
        0x1E3 => "pika_SPECIAL_S_HOLD",
        0x1E4 => "pika_SPECIAL_S_WEAK",
        0x1E5 => "pika_SPECIAL_S_ATTACK",
        0x1E6 => "pika_SPECIAL_S_END",
        0x1E7 => "pika_SPECIAL_LW_HIT",
        // The common kinds this moveset actually passes through.
        0 => "WAIT",
        1 => "WALK",
        3 => "DASH",
        6 => "TURN",
        7 => "TURN_DASH",
        11 => "FALL",
        14 => "FALL_AERIAL",
        22 => "ATTACK",
        23 => "ATTACK_100",
        28 => "ATTACK_AIR",
        55 => "CATCH",
        56 => "CATCH_PULL",
        57 => "CATCH_WAIT",
        64 => "THROW",
        475 => "SPECIAL_N",
        477 => "SPECIAL_S",
        478 => "SPECIAL_HI",
        479 => "SPECIAL_LW",
        _ => "",
    }
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
