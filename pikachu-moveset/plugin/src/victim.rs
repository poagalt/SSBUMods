// Reaching another fighter safely, and the list of engine statuses that are not ours to request.
//
// WHY THIS MODULE EXISTS. Every crash this moveset has produced has been the same mistake wearing
// different clothes: we reached across to another battle object, or handed one to engine code
// written for a different fighter, and something that exists for Incineroar or Donkey Kong and
// not for Pikachu got dereferenced. The Atmosphere reports say it plainly:
//
//   Data Abort, fault address 0
//     L2CFighterCommon::status_SwingGaogaenThrown + 0xf8
//     -> LinkModule__get_parent_pos_impl
//
// That is the whip's rope throw. `tools/disasm` on `lua2cpp_common.nro` -- pulled from the
// running game, 8792 defined symbols -- shows exactly why: `status_SwingGaogaenThrown` fetches
// the position of its LINKED PARENT and reads x, y and z off the returned pointer with no null
// check, because for Incineroar the parent is always his ROPE article. Pikachu has no rope, the
// link is absent, the pointer is null, and the third `ldr s0, [x0, #8]` is the fault address.
//
// The same disassembly shows the contrast that makes this a RULE rather than one bug:
// `status_SwingGaogaenCatched` calls `LinkModule::is_link` FIRST and only then touches the
// parent. That one is safe for us, and is safe for exactly the reason the other is not.
//
// So two things live here. A victim handle that is validated rather than assumed, and a refusal
// to request any status whose engine code needs an article we do not ship.
use smash::app::{lua_bind::*, sv_battle_object, BattleObjectModuleAccessor};
use smash::lib::lua_const::*;

/// A fighter's battle object id is `entry << 16`.
///
/// MEASURED, NOT GUESSED. The telemetry logged what the engine handed back during Pikachu's own
/// grab: `captured object id = 0x10000` while the victim was at entry 1. The obvious guess from
/// `BATTLE_OBJECT_ID_INVALID` (0x50000000) would have been `0x5000000n`, and it is wrong.
pub fn id_of_entry(entry: u32) -> u32 {
    entry << 16
}

/// Is this id shaped like a fighter's battle object id at all?
///
/// Cheap, and it catches the failure that matters: an id read from the engine at the wrong
/// moment. `CatchModule::capture_object_id` answers BATTLE_OBJECT_ID_INVALID once the capture has
/// ended, and a stale or invalid id handed to `sv_battle_object::module_accessor` produces a
/// pointer that is not an accessor -- and the first virtual call through it is an Instruction
/// Abort at a garbage address, which is the least legible crash this project has had.
pub fn plausible(id: u32) -> bool {
    id != *BATTLE_OBJECT_ID_INVALID as u32 && (id & 0xFFFF) == 0 && (id >> 16) < 8
}

/// The module accessor for a battle object id, or None if anything about it is wrong.
///
/// THE LAST CHECK IS THE LOAD-BEARING ONE. A `BattleObjectModuleAccessor` begins with its vtable
/// and then its own `battle_object_id`. Asking the accessor who it thinks it is, and requiring
/// the answer to match the id we looked up, rejects a pointer that is stale, recycled or simply
/// not an accessor -- which no amount of id arithmetic can do on its own.
///
/// # Safety
/// The caller must be on the game thread, inside a fighter callback.
pub unsafe fn boma(id: u32) -> Option<*mut BattleObjectModuleAccessor> {
    if !plausible(id) || sv_battle_object::is_null(id) {
        return None;
    }
    let acc = sv_battle_object::module_accessor(id);
    if acc.is_null() || (*acc).battle_object_id != id {
        return None;
    }
    Some(acc)
}

/// Every live fighter that is not `me`, as `(battle object id, accessor)`.
pub unsafe fn others(me: u32) -> Vec<(u32, *mut BattleObjectModuleAccessor)> {
    let mut out = Vec::new();
    for e in 0..8u32 {
        let id = id_of_entry(e);
        if id == me {
            continue;
        }
        if let Some(acc) = boma(id) {
            out.push((id, acc));
        }
    }
    out
}

/// Statuses whose engine code needs an article Pikachu does not have, and so must never be
/// requested on anybody.
///
/// READ OUT OF `lua2cpp_common.nro`, NOT GUESSED. Each of the three Alolan Whip rope statuses
/// calls into the linked parent -- the rope -- and `SwingGaogaenThrown` does it with no
/// `is_link` guard at all, which is the crash. They are listed by number rather than by constant
/// because the point is the ENGINE's status kinds, and a constant would suggest there is a
/// version of this we could safely enable.
///
///   0x177  SWING_GAOGAEN_THROWN       LinkModule::get_parent_pos, unguarded -- CONFIRMED CRASH
///   0x178  SWING_GAOGAEN_ATTACH_ROPE  the rope is the whole status
///   0x179  SWING_GAOGAEN_RETURN       rebound off the rope
///
/// SWING_GAOGAEN_CATCHED (0x176) is deliberately NOT here: it guards every parent access behind
/// `LinkModule::is_link`, so with no rope it simply does not constrain the victim's model, which
/// is exactly what we want while driving their position ourselves.
/// Unused in code ON PURPOSE: nothing requests these any more, which is the point. The list is
/// the machine-readable half of ADR 0004 and tools/check-victim.sh asserts nobody goes around it,
/// so deleting it would delete the rule.
#[allow(dead_code)]
pub const FORBIDDEN_VICTIM_STATUSES: &[i32] = &[0x177, 0x178, 0x179];

// ============================================================================================
// OWNERSHIP OF A HELD VICTIM IS OURS TO TRACK. THE ENGINE DOES NOT KEEP IT FOR US.
// ============================================================================================
//
// This is the same bug three times, and the log finally says so in one line:
//
//   [rat] WHIP p0 CAUGHT 0x10000 via in-range on f=16.0 (is_catch=false cap=0x50000000)
//   [rat] WHIP p0 rope moment: victim 0x10000 stays in status 0x44
//   [rat] WHIP p0 victim escaped 1 frame(s) after the whip -- ending
//
// The victim is in status 0x44 -- whatever the pull-in hitbox left them in -- NOT
// SWING_GAOGAEN_CATCHED, and `cap=false by=0x50000000` on every frame of every attempt says the
// engine holds no capture link either. Of course it does not: the engine did not make this grab.
// It refused to, which is the finding from two rounds ago, so the catch was taken geometrically
// and there is nobody but us who thinks the victim is held.
//
// Every "is the victim still ours" test in this moveset was asking the engine:
//
//   the whip   status == SWING_GAOGAEN_CATCHED, or a capture link naming us
//   the carry  one of the CAPTURE_* statuses, or a capture link naming us
//
// Both are false immediately, so both features cancel themselves. The whip exits to
// SPECIAL_S_END the frame after the whip -- reported as "it cancels and a failure animation
// plays" -- and the carry drops the victim as soon as the engine's own throw finishes, reported
// as "the opponent falls off Pikachu's shoulder after a second".
//
// So ownership lives here, as a fact we assert and clear ourselves. The engine's link is still
// honoured when it exists, because for an ordinary grab it does and it is better than our
// bookkeeping -- but its absence is no longer evidence of anything.
//
// A HOLD MUST EXPIRE. A flag that only we clear is a stuck grab waiting to happen, which is the
// one failure worse than not grabbing at all. `special_s_catch_clatter_frame` is 85 frames of
// mashing, so twice that is a ceiling no legitimate hold reaches and every broken one does.

/// Ceiling on a hold, in frames. Twice Incineroar's 85-frame clatter window.
pub const MAX_HOLD_FRAMES: i32 = 170;

static mut HELD_ID: [u32; 8] = [0; 8];
static mut HELD_FOR: [i32; 8] = [0; 8];

/// Take ownership of a victim. Returns false if the id is not a live fighter.
pub unsafe fn hold(entry: usize, id: u32) -> bool {
    if entry >= 8 || boma(id).is_none() {
        return false;
    }
    HELD_ID[entry] = id;
    HELD_FOR[entry] = 0;
    crate::telemetry::sink::crumb(format!("[rat] HOLD p{entry} took {id:#x}"));
    true
}

/// Give them up.
pub unsafe fn release(entry: usize, why: &str) {
    if entry >= 8 || HELD_ID[entry] == 0 {
        return;
    }
    crate::telemetry::sink::crumb(format!(
        "[rat] HOLD p{entry} released {:#x} after {}f: {why}",
        HELD_ID[entry], HELD_FOR[entry]
    ));
    HELD_ID[entry] = 0;
    HELD_FOR[entry] = 0;
}

/// The victim we are holding, if we still are.
///
/// Ours to answer, with three ways to lose them and none of them being "the engine stopped
/// agreeing": the object went away, the hold timed out, or somebody called `release`.
pub unsafe fn held(entry: usize) -> Option<(u32, *mut BattleObjectModuleAccessor)> {
    if entry >= 8 || HELD_ID[entry] == 0 {
        return None;
    }
    let id = HELD_ID[entry];
    match boma(id) {
        Some(acc) => Some((id, acc)),
        None => {
            release(entry, "the victim is no longer a live battle object");
            None
        }
    }
}

/// How long we have held them.
#[allow(dead_code)]
pub unsafe fn held_frames(entry: usize) -> i32 {
    if entry >= 8 { 0 } else { HELD_FOR[entry] }
}

/// Advance the hold clock. Called once per frame from the telemetry hook, next to `new_frame`,
/// so a hold cannot outlive its ceiling even if the move that created it stops running.
pub unsafe fn tick_hold(entry: usize) {
    if entry >= 8 || HELD_ID[entry] == 0 {
        return;
    }
    HELD_FOR[entry] += 1;
    if HELD_FOR[entry] > MAX_HOLD_FRAMES {
        release(entry, "hold ceiling reached -- something never let go");
    }
}

/// How many status changes one entry may make in a single game frame before something is wrong.
///
/// A HARD STOP ON THE WHOLE CLASS OF FREEZE, not on one instance of it.
///
/// The cargo carry froze the game with 7965 status changes inside one frame: a line callback
/// whose trigger stayed true after the transition it caused, so it requested the status it was
/// already in, whose Main ran the callback again. Nothing in that loop is unusual -- it is two
/// ordinary pieces of code, each correct on its own -- and the same shape is available to every
/// state machine in this moveset. The whip has six statuses that can reach each other, the carry
/// has ten, and any pair whose conditions disagree by a frame is a freeze.
///
/// A freeze is the worst failure mode available: no crash report, no log flush, no input, and the
/// only recovery is closing the game. So it is converted into a logged bailout. Eight is far more
/// than any legitimate chain needs -- the longest real one is grab to carry to throw, which is
/// three -- and far less than a hang.
pub const MAX_SHIFTS_PER_FRAME: u32 = 8;

static mut SHIFTS: [u32; 8] = [0; 8];

/// Call once per frame, per entry, from a line callback. Resets the budget.
///
/// If the budget is being exceeded the frame never ends, so this never runs -- which is exactly
/// the property that makes the counter a reliable detector rather than a heuristic.
pub unsafe fn new_frame(entry: usize) {
    if entry < 8 {
        SHIFTS[entry] = 0;
    }
}

/// Charge one status change against this frame's budget. False means refuse it.
pub unsafe fn allow_shift(entry: usize, what: &str) -> bool {
    if entry >= 8 {
        return true;
    }
    SHIFTS[entry] += 1;
    if SHIFTS[entry] <= MAX_SHIFTS_PER_FRAME {
        return true;
    }
    // Only report the first refusal. If something really is looping, the log should say so once
    // and not become the new denial of service.
    if SHIFTS[entry] == MAX_SHIFTS_PER_FRAME + 1 {
        crate::telemetry::sink::crumb(format!(
            "[rat] SHIFT STORM p{entry}: more than {MAX_SHIFTS_PER_FRAME} status changes in one \
             frame, refusing '{what}' and everything after it this frame. Two pieces of code are \
             disagreeing about which status we should be in -- find them; without this the game \
             would have frozen."
        ));
    }
    false
}

/// Request a status on another fighter, refusing the ones that would crash.
///
/// The refusal is logged rather than silent. A port that quietly declines to do something is how
/// a move ends up half-working for a session; a line in the log saying which status was refused
/// and why is how the next reader knows the trajectory is being driven by hand on purpose.
///
/// # Safety
/// `vboma` must be a live accessor, ideally one that came from [`boma`].
/// Kept for the next port that needs to move a victim: it is the only safe way to do it, and
/// re-deriving the refusal list at that point is how ADR 0004 gets violated by accident.
#[allow(dead_code)]
pub unsafe fn request_status(
    vboma: *mut BattleObjectModuleAccessor,
    status: i32,
    who: &str,
) -> bool {
    if FORBIDDEN_VICTIM_STATUSES.contains(&status) {
        crate::telemetry::sink::crumb(format!(
            "[rat] REFUSED to put a victim into status {status:#x} from {who} -- it dereferences \
             Incineroar's rope article through LinkModule and there is no rope; see src/victim.rs"
        ));
        return false;
    }
    crate::telemetry::sink::crumb(format!(
        "[rat] about to request status {status:#x} on victim from {who}"
    ));
    StatusModule::change_status_request_from_script(vboma, status, false);
    true
}
