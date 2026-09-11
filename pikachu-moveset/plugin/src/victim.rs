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
pub const FORBIDDEN_VICTIM_STATUSES: &[i32] = &[0x177, 0x178, 0x179];

/// Request a status on another fighter, refusing the ones that would crash.
///
/// The refusal is logged rather than silent. A port that quietly declines to do something is how
/// a move ends up half-working for a session; a line in the log saying which status was refused
/// and why is how the next reader knows the trajectory is being driven by hand on purpose.
///
/// # Safety
/// `vboma` must be a live accessor, ideally one that came from [`boma`].
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
