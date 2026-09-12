// WATCHES THE ENGINE HOLD A VICTIM, so the port can copy a recording instead of a guess.
//
// WHY THIS, AFTER EVERYTHING ELSE. Six rounds of grab bugs have each been the same shape: the
// native code that decides "this catch connected, so put the victim there and me here" is compiled
// C++ in `main`, keyed on the fighter kind, and it does not run for a Pikachu. Every attempt to
// reproduce it has been assembled from static reading -- the fighter NRO, lua2cpp_common,
// lua_dict, the param prcs -- and static reading cannot see what the native code DOES, only what
// it is made of. The last three rounds each fixed a real thing and each left the grab broken in a
// new way, which is what inference looks like when it is nearly right.
//
// The game is running. Donkey Kong and Incineroar are playable. The native code is not readable
// but it is OBSERVABLE: hook the module functions it calls, log every call with its arguments and
// its caller, and one cargo throw by the real Donkey Kong produces the exact sequence, in order,
// with real values. That is not an approximation of the native logic; it IS the native logic,
// recorded. Then the port becomes transcription.
//
// WHAT IS HOOKED, AND WHAT DELIBERATELY IS NOT. Only calls that CHANGE capture state, which fire
// a handful of times per grab:
//
//   CatchModule     set_catch, catch_cut
//   CaptureModule   capture, capture_2, capture_cut, set_nodes, motion, set_ignore_catching
//   LinkModule      send_event_nodes, send_event_nodes_throw, set_model_constraint_pos_ort, unlink
//   ControlModule   start_clatter
//   StatusModule    change_status_request_from_script
//
// The queries -- `is_link`, `is_capture`, `catch_object_id`, `get_parent_id` -- are NOT hooked.
// They are called every frame by every object and would bury the signal; and what they return is
// already visible from `crate::victim::trace`.
//
// EVERY CALLER IS TAGGED, because the whole point is the difference between fighters. A line says
// which fighter kind and which entry made the call, so a Donkey Kong recording and a Pikachu
// recording can be diffed directly. Hash40 arguments are logged raw and resolved offline with
// `tools/hash40.sh` -- resolving them in-game would need the name list on the console.
//
// OFF BY DEFAULT. It is an instrument, not telemetry: flip ENABLED, deploy, play the SOURCE
// fighter, and read the log.
use smash::app::{lua_bind::*, utility, BattleObjectModuleAccessor, KineticUtility};
use smash::lib::lua_const::*;
use smash::phx::Hash40;

use super::sink;

/// Whether to record. A deliberate one-off, like `exedump`.
pub const ENABLED: bool = true;

/// Cap on recorded lines, so a forgotten `ENABLED` cannot fill the card.
const MAX_LINES: u32 = 4000;
static mut LINES: u32 = 0;

/// `pika0`, `DONKEY1`, `GAOGAEN0` -- who made the call.
unsafe fn who(boma: *mut BattleObjectModuleAccessor) -> String {
    if boma.is_null() {
        return "<null>".to_string();
    }
    let kind = utility::get_kind(&mut *boma);
    let tag = if kind == *FIGHTER_KIND_PIKACHU {
        "pika"
    } else if kind == *FIGHTER_KIND_GAOGAEN {
        "GAOGAEN"
    } else if kind == *FIGHTER_KIND_DONKEY {
        "DONKEY"
    } else if kind == *FIGHTER_KIND_GAMEWATCH {
        "GAMEWATCH"
    } else if kind == *FIGHTER_KIND_MARIO {
        "mario"
    } else {
        "other"
    };
    // The object id carries the entry: `(id >> 16) & 0xfff`, read out of
    // `get_battle_object_from_id` rather than assumed.
    let id = (*boma).battle_object_id;
    // THE STATUS IS PART OF THE IDENTITY, because the questions this instrument answers are about
    // branchy code. `FUN_710001d4b0`, the kinetic setup all six of Incineroar's side-B statuses
    // call, dispatches on the current status and then again on the situation -- and which branch
    // runs cannot be read reliably out of 748 lines of decompiled C with confidence, but it is
    // obvious the moment each call says which status it came from.
    format!(
        "{tag}{} st={:#x} mot={:#x}",
        (id >> 16) & 0xfff,
        StatusModule::status_kind(boma),
        MotionModule::motion_kind(boma)
    )
}

unsafe fn rec(line: String) {
    if !ENABLED || LINES >= MAX_LINES {
        return;
    }
    LINES += 1;
    sink::diag(format!("[rat] TRACE {line}"));
}

// ---- CatchModule ----------------------------------------------------------------------------

#[skyline::hook(replace = CatchModule::set_catch)]
unsafe fn set_catch_hook(boma: *mut BattleObjectModuleAccessor, id: u32) -> u64 {
    rec(format!(
        "{} CatchModule::set_catch({id:#x}){}",
        who(boma),
        if id == *BATTLE_OBJECT_ID_INVALID as u32 { "   <- clearing" } else { "   <- CLAIMING" }
    ));
    original!()(boma, id)
}

#[skyline::hook(replace = CatchModule::catch_cut)]
unsafe fn catch_cut_hook(boma: *mut BattleObjectModuleAccessor, a: bool, b: bool) -> u64 {
    rec(format!("{} CatchModule::catch_cut({a}, {b})", who(boma)));
    original!()(boma, a, b)
}

// ---- CaptureModule --------------------------------------------------------------------------

#[skyline::hook(replace = CaptureModule::capture)]
unsafe fn capture_hook(
    boma: *mut BattleObjectModuleAccessor,
    id: u32,
    node: i32,
    flag: bool,
    last: i32,
) -> u64 {
    rec(format!(
        "{} CaptureModule::capture(captor={id:#x}, arg3={node:#x}, {flag}, {last})",
        who(boma)
    ));
    original!()(boma, id, node, flag, last)
}

#[skyline::hook(replace = CaptureModule::capture_2)]
unsafe fn capture_2_hook(
    boma: *mut BattleObjectModuleAccessor,
    id: u32,
    a: i32,
    b: i32,
    flag: bool,
    last: i32,
) -> u64 {
    rec(format!(
        "{} CaptureModule::capture_2(captor={id:#x}, {a:#x}, {b:#x}, {flag}, {last})",
        who(boma)
    ));
    original!()(boma, id, a, b, flag, last)
}

#[skyline::hook(replace = CaptureModule::capture_cut)]
unsafe fn capture_cut_hook(
    boma: *mut BattleObjectModuleAccessor,
    a: bool,
    b: bool,
    c: bool,
) -> u64 {
    rec(format!("{} CaptureModule::capture_cut({a}, {b}, {c})", who(boma)));
    original!()(boma, a, b, c)
}

#[skyline::hook(replace = CaptureModule::set_nodes)]
unsafe fn set_nodes_hook(
    boma: *mut BattleObjectModuleAccessor,
    a: Hash40,
    b: Hash40,
    f: f32,
) -> u64 {
    rec(format!(
        "{} CaptureModule::set_nodes({:#x}, {:#x}, {f})",
        who(boma),
        a.hash,
        b.hash
    ));
    original!()(boma, a, b, f)
}

#[skyline::hook(replace = CaptureModule::motion)]
unsafe fn capture_motion_hook(
    boma: *mut BattleObjectModuleAccessor,
    m: Hash40,
    i: i32,
) -> u64 {
    rec(format!("{} CaptureModule::motion({:#x}, {i})", who(boma), m.hash));
    original!()(boma, m, i)
}

#[skyline::hook(replace = CaptureModule::set_ignore_catching)]
unsafe fn set_ignore_catching_hook(boma: *mut BattleObjectModuleAccessor, on: bool) -> u64 {
    rec(format!("{} CaptureModule::set_ignore_catching({on})", who(boma)));
    original!()(boma, on)
}

// ---- LinkModule -----------------------------------------------------------------------------

#[skyline::hook(replace = LinkModule::send_event_nodes)]
unsafe fn send_event_nodes_hook(
    boma: *mut BattleObjectModuleAccessor,
    link_no: i32,
    event: Hash40,
    arg: u32,
) -> u64 {
    rec(format!(
        "{} LinkModule::send_event_nodes(link={link_no}, event={:#x}, {arg})",
        who(boma),
        event.hash
    ));
    original!()(boma, link_no, event, arg)
}

#[skyline::hook(replace = LinkModule::send_event_nodes_throw)]
unsafe fn send_event_nodes_throw_hook(
    boma: *mut BattleObjectModuleAccessor,
    event: Hash40,
    motion: Hash40,
    flag: bool,
    a: i32,
    b: i32,
    c: i32,
) -> u64 {
    rec(format!(
        "{} LinkModule::send_event_nodes_throw(event={:#x}, motion={:#x}, {flag}, {a:#x}, {b:#x}, {c:#x})",
        who(boma),
        event.hash,
        motion.hash
    ));
    original!()(boma, event, motion, flag, a, b, c)
}

#[skyline::hook(replace = LinkModule::unlink)]
unsafe fn unlink_hook(boma: *mut BattleObjectModuleAccessor, link_no: i32) -> u64 {
    rec(format!("{} LinkModule::unlink(link={link_no})", who(boma)));
    original!()(boma, link_no)
}

// ---- ControlModule --------------------------------------------------------------------------

#[skyline::hook(replace = ControlModule::start_clatter)]
unsafe fn start_clatter_hook(
    boma: *mut BattleObjectModuleAccessor,
    frame: f32,
    dec: f32,
    mul: f32,
    a: i8,
    b: i32,
    c: bool,
    d: bool,
) -> u64 {
    rec(format!(
        "{} ControlModule::start_clatter({frame}, {dec}, {mul}, {a}, {b}, {c}, {d})",
        who(boma)
    ));
    original!()(boma, frame, dec, mul, a, b, c, d)
}

pub fn install() {
    if !ENABLED {
        return;
    }
    skyline::install_hooks!(
        set_catch_hook,
        catch_cut_hook,
        capture_hook,
        capture_2_hook,
        capture_cut_hook,
        set_nodes_hook,
        capture_motion_hook,
        set_ignore_catching_hook,
        send_event_nodes_hook,
        send_event_nodes_throw_hook,
        unlink_hook,
        start_clatter_hook,
        change_kinetic_hook,
        enable_energy_hook,
        unable_energy_hook,
        clear_unable_energy_hook
    );
    sink::diag(
        "[rat] TRACE engine capture tracer ARMED -- play the SOURCE fighter and every call that \
         changes capture state will be recorded with its caller and arguments"
            .to_string(),
    );
}

// ---- the kinetics ---------------------------------------------------------------------------
//
// WHY THESE, AND WHY LATE. The whip's movement is one function -- `FUN_710001d4b0` at gaogaen
// 0x710001d4b0 -- that every one of Incineroar's six side-B statuses calls. It is 62 engine calls
// and no instance-work writes, so it is transcribable in principle, and it dispatches on the
// current status and then on the situation through a nest of conditions that cannot be read out of
// 748 lines of Ghidra output with any confidence.
//
// ADR 0006 says a transcription must be a copy, and a copy of a branch tree I have guessed at is
// the reimplementation this whole rewrite exists to stop. Which branch RUNS is not a deduction
// though -- it is a recording, and that is what solved the capture. So: hook the kinetic calls,
// tag each with the status it came from, play one side-B, and transcribe the executed path.
//
// `sv_kinetic_energy::reset_energy` and friends are deliberately absent: they are called through
// the lua stack, so a hook on them sees a `lua_State` and none of the arguments. The direct
// module calls below carry theirs, and the reset TYPE is visible in the static source where the
// branch structure is not.

#[skyline::hook(replace = KineticModule::change_kinetic)]
unsafe fn change_kinetic_hook(boma: *mut BattleObjectModuleAccessor, kind: i32) -> u64 {
    rec(format!("{} KineticModule::change_kinetic({kind:#x})", who(boma)));
    original!()(boma, kind)
}

#[skyline::hook(replace = KineticModule::enable_energy)]
unsafe fn enable_energy_hook(boma: *mut BattleObjectModuleAccessor, id: i32) -> u64 {
    rec(format!("{} KineticModule::enable_energy({id:#x})", who(boma)));
    original!()(boma, id)
}

#[skyline::hook(replace = KineticModule::unable_energy)]
unsafe fn unable_energy_hook(boma: *mut BattleObjectModuleAccessor, id: i32) -> u64 {
    rec(format!("{} KineticModule::unable_energy({id:#x})", who(boma)));
    original!()(boma, id)
}

/// The call that DISAMBIGUATES THE BRANCH, which is the only reason it is hooked.
///
/// `FUN_710001d4b0` contains two `change_kinetic(GROUND_STOP)` sites in different branches of its
/// bool argument, and a recording of `change_kinetic` alone cannot say which one fired -- both
/// produce the same kinetic type from the same status. Picking one would be assembling the move
/// rather than copying it, which ADR 0006 exists to forbid.
///
/// The two blocks differ in their `clear_unable_energy` signature, and that is decisive:
///
///     0x710001dbfc..dc84   MOTION, GRAVITY, CONTROL
///     0x710001e408..e490   GRAVITY, CONTROL, STOP
///
/// So one recorded triple names the block outright. `KineticUtility::clear_unable_energy` takes
/// its arguments directly rather than through the lua stack, which is what makes it readable where
/// `sv_kinetic_energy::reset_energy` is not -- and once the block is known, the reset types and
/// speeds in it are already legible in the static source.
///
/// Note the argument order: the energy id comes FIRST and the accessor second, which is the
/// opposite of every other call in this file.
#[skyline::hook(replace = KineticUtility::clear_unable_energy)]
unsafe fn clear_unable_energy_hook(id: i32, boma: *mut BattleObjectModuleAccessor) {
    rec(format!("{} KineticUtility::clear_unable_energy({id:#x})", who(boma)));
    original!()(id, boma)
}
