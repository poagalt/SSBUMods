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
///   0x177  SWING_GAOGAEN_THROWN       LinkModule::get_parent_pos, unguarded -- was a real crash
///   0x178  SWING_GAOGAEN_ATTACH_ROPE  the rope is the whole status
///   0x179  SWING_GAOGAEN_RETURN       rebound off the rope
///
/// TWO OF THE THREE ARE NOW SAFE, AND THE REASON IS THAT THE LINK WAS THE PROBLEM ALL ALONG.
///
/// 0x177's entry above is accurate about the code: `status_SwingGaogaenThrown` (common
/// 0x710028c430) opens with `LinkModule::get_parent_pos(x, y, z, LINK_NO_CAPTURE)` and only reaches
/// `is_link` several calls later. With no parent that is a null read, which is the crash this list
/// was written for. But the missing parent was never about the rope -- it was `LinkModule::link`
/// being called on one object instead of two, so the victim had no parent at all. With both ends
/// linked the engine itself put the victim into 0x177 on the last playtest and nothing crashed;
/// `crate::victim::trace` logged `status=0x177 captured=true master=0x0` and then a normal exit.
///
/// 0x179 reads no parent whatsoever. `status_SwingGaogaenReturn` (0x710028f230) is `reverse_lr`,
/// a motion, four `clear_unable_energy`, and the STOP energy driven from
/// `FIGHTER_STATUS_SWING_GAOGAEN_WORK_FLOAT_RETURN_SPEED_X` -- a value the victim's own CATCHED
/// status already stored from the CAPTOR's `special_s_through_target_return_speed`. Nothing in it
/// needs a rope to exist.
///
/// 0x178 stays. The rope IS that status.
///
/// SWING_GAOGAEN_CATCHED (0x176) was never here: it guards every parent access behind
/// `LinkModule::is_link`.
pub const FORBIDDEN_VICTIM_STATUSES: &[i32] = &[0x178];

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
//
// EXCEPT THAT THE CARGO CARRY REACHES IT EVERY SINGLE TIME, and the measurement is unambiguous.
// Six carries in one session, and five of the six end here:
//
//   [rat] VICTIM p0 cargo: status=0xc5 captured=true master=0x0 clatter=426.0
//   [rat] VICTIM p0 cargo: status=0xc6 captured=true master=0x0 clatter=425.0
//   [rat] HOLD p0 asked the engine to cut the catch on 0x10000: hold ceiling reached
//   [rat] HOLD p0 released 0x10000 after 171f: hold ceiling reached -- something never let go
//
// 171 frames, every time, on a carry whose clatter timer had 425 of 426 frames left. Nothing
// "never let go" -- the ceiling let go, 255 frames early, and "after some time mario still gets
// stuck to pikachu's feet" is the tail of a carry being cut while the animation is mid-pose.
//
// SO THE CEILING IS NOW THE ENGINE'S NUMBER RATHER THAN A CONSTANT DERIVED FROM ONE OF THE TWO
// MOVES. `ControlModule::get_clatter_time` on the victim is what the engine's own escape check
// tests and what `CaptureWaitCommon` counts down; it reads 85 for the whip's
// SWING_GAOGAEN_CATCHED and 426 for the carry's SHOULDERED_DONKEY, both measured. Taking twice
// whatever the engine set at the moment of the hold keeps the original whip ceiling exactly
// (85 * 2 = 170) and stops the carry being cut at a third of its length.
//
// The flat floor stays for the case that matters most: a victim whose clatter reads 0 at the
// moment of the hold is a hold this module should not trust for long.

/// Floor on the hold ceiling, in frames. Twice Incineroar's 85-frame clatter window, which is
/// also what the derived ceiling comes to for the whip.
pub const MAX_HOLD_FRAMES: i32 = 170;

/// Per-entry ceiling, seeded from the victim's own clatter timer in `hold`.
static mut HOLD_CEILING: [i32; 8] = [MAX_HOLD_FRAMES; 8];

static mut HELD_ID: [u32; 8] = [0; 8];
static mut HELD_FOR: [i32; 8] = [0; 8];

/// Take ownership of a victim. Returns false if the id is not a live fighter.
pub unsafe fn hold(entry: usize, id: u32) -> bool {
    if entry >= 8 || boma(id).is_none() {
        return false;
    }
    HELD_ID[entry] = id;
    HELD_FOR[entry] = 0;
    // Seeded from the engine's own mash timer for THIS grab -- see the note on MAX_HOLD_FRAMES.
    // A grab the engine has not set a clatter for reads 0 and gets the flat floor.
    let clatter = match boma(id) {
        Some(vboma) => smash::app::lua_bind::ControlModule::get_clatter_time(vboma, 0),
        None => 0.0,
    };
    let derived = (clatter * 2.0) as i32;
    HOLD_CEILING[entry] = derived.max(MAX_HOLD_FRAMES);
    crate::telemetry::sink::crumb(format!(
        "[rat] HOLD p{entry} took {id:#x}; their clatter is {clatter} so this hold expires after \
         {}f if nothing else ends it",
        HOLD_CEILING[entry]
    ));
    true
}

/// Give them up.
pub unsafe fn release(entry: usize, why: &str) {
    if entry >= 8 || HELD_ID[entry] == 0 {
        return;
    }
    // THE ENGINE'S HALF OF THE RELEASE, from the one place every path goes through.
    //
    // Anything that established a real capture -- `specials::take_captive` for the whip, Pikachu's
    // own grab for the cargo -- has to undo it, or the victim stays captured after we have
    // forgotten them. The call is the game's: `FighterStatusUniqProcessCaptureMasterhand_exit_status`
    // and its siblings all cut with `CaptureModule::capture_cut(boma, false, false, false)` on the
    // CAPTURED object's own module.
    //
    // Guarded by who the engine says owns them, so this can never cut somebody else's grab: the
    // captured object reports its captor through `catch_object_id`, and it has to be us.
    cut_capture(entry, why);
    forget(entry, why);
}

/// Drop our bookkeeping WITHOUT asking the engine to cut anything.
///
/// FOR THE PATH THAT HAS ALREADY CUT, AND CUTTING TWICE IS NOT FREE. `CatchModule::catch_cut` is
/// not an action, it is a REQUEST -- Ghidra on `CatchCut` shows four instructions writing
/// `this->[0x93] = 1`, and the engine does the teardown on its next update. DK issues exactly one,
/// from his throws' `EXIT_STATUS` line. This port was issuing two per throw: one from
/// `throw_exit`, which is DK's, and one from `throw_loop` through `release` -> `cut_capture`,
/// which is not.
///
/// A request flag raised when there is nothing to cut is a request that survives to the next
/// thing there is, and the measured log is that shape exactly -- five cargo throws that worked
/// interleaved with grabs that played the whole catch animation and latched onto nobody:
///
///     MOVE p0 grab: CATCH(0x37) 51f -> WAIT(0x0)
///     MOVE p0 grab: CATCH(0x37) 35f -> WAIT(0x0)
///     MOVE p0 grab: CATCH(0x37) 69f -> WAIT(0x0)
///
/// with `cap=false vst=0x0` on the victim throughout, so nothing was still holding him. The
/// engine's own catch was being cut as it started.
pub unsafe fn forget(entry: usize, why: &str) {
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
    // THE CLATTER AT `hold()` IS THE WRONG MOMENT, AND THE CARRY STILL DIED AT 171 FRAMES.
    //
    // Seeding the ceiling once, in `hold`, reads the mash timer of the ORDINARY GRAB the carry is
    // intercepted out of -- 55, 68, 105 in the measured log -- and then the carry starts, the
    // `hash40("shoulder")` event moves the victim into SHOULDERED_DONKEY, and the engine RE-ARMS
    // the clatter to a much larger number for the carry:
    //
    //     HOLD p0 took 0x10000; their clatter is 68 so this hold expires after 170f
    //     VICTIM p0 cargo: status=0xc5 captured=true clatter=105.0
    //     VICTIM p0 cargo: status=0xc6 captured=true clatter=104.0
    //     SHOULDER p0 jump (button) out of the carry after 168f
    //     HOLD p0 released 0x10000 after 171f: hold ceiling reached
    //
    // The jump at frame 168 looked like it broke the carry; the ceiling ended it three frames
    // later for reasons that had nothing to do with jumping.
    //
    // So the ceiling is re-derived every frame and only ever moves FORWARD: while the engine still
    // has mash time left on this victim, the deadline stays ahead of us, and the moment the clatter
    // reaches zero it stops moving and we expire. That is the engine's own end-of-hold condition
    // -- `CaptureWaitCommon` escapes on `get_clatter_time <= 0` -- with a bound rather than a
    // trust: a victim whose clatter never decrements still cannot be held past twice its value.
    if let Some(vboma) = boma(HELD_ID[entry]) {
        let clatter = smash::app::lua_bind::ControlModule::get_clatter_time(vboma, 0);
        if clatter > 0.0 {
            let deadline = HELD_FOR[entry] + (clatter * 2.0) as i32;
            if deadline > HOLD_CEILING[entry] {
                HOLD_CEILING[entry] = deadline;
            }
        }
    }
    if HELD_FOR[entry] > HOLD_CEILING[entry] {
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
        "[rat] about to force status {status:#x} on victim from {who}"
    ));
    // FORCED, BECAUSE AN UNFORCED REQUEST FROM ANOTHER FIGHTER'S SCRIPT DOES NOT ARRIVE.
    //
    // This was `change_status_request_from_script(vboma, status, false)`, and the whip's victim
    // ended up captured but never transitioned: the playtest reported Mario "stuck to pikachu's
    // feet" through movement and jumping until an attack freed him, and the diagnostic agreed --
    //
    //     caught 0x10000: is_capture=true ours_link=true their_is_link=true their_st=0x0
    //
    // `their_st=0x0` is FIGHTER_STATUS_KIND_WAIT. He was constrained to the captor (hence sitting
    // at the captor's ORIGIN, which is between the feet) with his own status untouched, so nothing
    // drove his motion, nothing started his clatter, and nothing could release him.
    //
    // A `_from_script` request is the polite form: it is queued and the engine may decline it
    // against the victim's own transition table, and WAIT -> SWING_GAOGAEN_CATCHED is not a
    // transition any fighter's own script is expected to make. When the engine performs a real
    // catch it does not ask -- it changes the victim's status itself. Since this path exists
    // precisely because the engine declined to do that, it has to be equally direct.
    StatusModule::change_status_force(vboma, status, false);
    true
}

// ---------------------------------------------------------------------------------------------
// WHAT THE ENGINE IS DOING TO THE VICTIM, ONE LINE PER CHANGE.
// ---------------------------------------------------------------------------------------------

/// The victim's engine-side state, logged only when it changes.
///
/// WHY THIS IS THE MISSING MEASUREMENT. Both remaining grab bugs are reports about the VICTIM --
/// "the opponent can still attack while the grab is happening", "mario escapes a brief moment
/// after being grabbed no matter the percent" -- and everything this plugin logged was about US.
/// Six rounds of fixes were aimed at our side of a release that the engine was performing on
/// theirs.
///
/// The disassembly says what to watch. Both the whip's victim status and the cargo's end the same
/// way, in code neither we nor Incineroar nor Donkey Kong wrote:
///
///   status_SwingGaogaenCatched_Main:  if ControlModule::get_clatter_time(boma) <= 0.0 -> escape
///   CaptureWaitCommon:                if ControlModule::get_clatter_time(boma) <= 0.0 -> escape
///
/// A clatter time that starts at zero and a clatter time that runs out are the same symptom from
/// our side and completely different bugs, and this is the only line that can tell them apart.
///
/// One line per CHANGE, not per frame: a carry is a few hundred frames and the interesting events
/// are the status transitions and the frame the clatter dies.
///
/// # Safety
/// Game thread, inside a fighter callback, with an accessor from [`boma`].
pub unsafe fn trace(entry: usize, vboma: *mut BattleObjectModuleAccessor, who: &str) {
    if entry >= 8 {
        return;
    }
    let status = StatusModule::status_kind(vboma);
    let captured = CaptureModule::is_capture(vboma);
    let master = CaptureModule::catch_object_id(vboma) as u32;
    // Quantised, because it decrements every frame and only the crossings matter: still running,
    // or run out.
    let clatter = ControlModule::get_clatter_time(vboma, 0);
    let alive = clatter > 0.0;
    let now = (status, captured, master, alive);
    if LAST_SEEN[entry] == Some(now) {
        return;
    }
    LAST_SEEN[entry] = Some(now);
    crate::telemetry::sink::diag(format!(
        "[rat] VICTIM p{entry} {who}: status={status:#x} captured={captured} master={master:#x} \
         clatter={clatter:.1} (<=0 is what the engine's escape check tests)"
    ));
}

/// Forgets the last traced state, so the next carry or catch logs its opening state again.
pub fn untrace(entry: usize) {
    unsafe {
        if entry < 8 {
            LAST_SEEN[entry] = None;
        }
    }
}

static mut LAST_SEEN: [Option<(i32, bool, u32, bool)>; 8] = [None; 8];

/// Cuts an engine capture we established, and only one we established.
///
/// Called from [`release`], so every path that lets go performs both halves -- and separately by
/// Alolan Whip when its hand-driven flight starts, because the engine's model constraint and a
/// `PostureModule::set_pos` every frame are two writers fighting over one transform. That was the
/// leading suspect for the cargo's unexplained Instruction Abort and the lesson is written into
/// `shoulder::hold_victim`: whoever is placing the victim, it must be exactly one of us.
///
/// The guard is the ENGINE's opinion of who owns them, not ours. A captured object reports its
/// captor through `CaptureModule::catch_object_id`, and it has to be us -- so this can never cut
/// another fighter's grab, which matters because Pikachu's own throw uses the same machinery.
///
/// Returns whether anything was cut, so a caller can log the difference between "released the
/// engine's hold" and "there was nothing to release".
pub unsafe fn cut_capture(entry: usize, why: &str) -> bool {
    if entry >= 8 || HELD_ID[entry] == 0 {
        return false;
    }
    let id = HELD_ID[entry];
    let Some(vboma) = boma(id) else { return false };
    if !CaptureModule::is_capture(vboma) {
        return false;
    }
    if CaptureModule::catch_object_id(vboma) as u32 != id_of_entry(entry as u32) {
        return false;
    }
    // THE RELEASE IS `CatchModule::catch_cut` ON US, NOT `capture_cut` ON THEM.
    //
    // RECORDED, NOT REASONED. src/telemetry/enginetrace.rs hooks the twelve calls that change
    // capture state and logs them with their caller; one session of the real Donkey Kong and the
    // real Incineroar produced 533 of them, and across every grab and every cargo throw:
    //
    //   CaptureModule::capture_cut      0 calls by either fighter
    //   CatchModule::catch_cut          what Donkey Kong ends every carry with, (false, false)
    //
    // `capture_cut` was the wrong function entirely, and Ghidra says why it glued rather than
    // released: `CaptureCut(this, p2, p3, p4)` gates its whole teardown on `p2`, so the
    // `(false, false, false)` this used to pass skipped the two vtable calls -- 0x3b8 and 0x3c0 --
    // that undo the capture. The victim's model stayed constrained to our `"throw"` bone, which
    // is "modified pikachu and mario end up glued together".
    //
    // `CatchCut` is four instructions: `this->[0x93] = 1`. It is a REQUEST, and the engine does
    // the whole teardown itself on its next update, which is exactly why it cannot leave a
    // constraint behind.
    let Some(ours) = boma(id_of_entry(entry as u32)) else { return false };
    CatchModule::catch_cut(ours, false, false);
    crate::telemetry::sink::crumb(format!(
        "[rat] HOLD p{entry} asked the engine to cut the catch on {id:#x}: {why}"
    ));
    true
}

/// Is the engine holding them for us right now?
///
/// The question every cross-object position write has to ask first. While this is true the
/// victim's own `SwingGaogaenCatched` is running the model constraint, their facing and their
/// clatter, and the correct thing for this plugin to do about their position is nothing.
pub unsafe fn engine_holds(entry: usize, vboma: *mut BattleObjectModuleAccessor) -> bool {
    entry < 8
        && CaptureModule::is_capture(vboma)
        && CaptureModule::catch_object_id(vboma) as u32 == id_of_entry(entry as u32)
}
