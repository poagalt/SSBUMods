// One line per move attempt, saying which statuses it passed through and for how long.
//
// WHY. A real session's log is five megabytes. The per-frame probes in it are what actually
// diagnosed the last three bugs and they are staying -- but reading "did side-B do the right
// thing" out of five megabytes means already knowing which frames to look at, which is the thing
// you do not know when a playtest report says "the whip is back to not grabbing".
//
// So the trail is recorded as it happens and summarised when the move ends:
//
//   [rat] MOVE p0 side-B: SPECIAL_S 17f -> SPECIAL_S_THROW 41f -> SPECIAL_S_SHOULDER 30f -> WAIT
//   [rat] MOVE p0 down-B: SPECIAL_LW 15f -> SPECIAL_LW_HIT 67f -> FALL
//   [rat] MOVE p0 cargo: CATCH 7f -> CATCH_WAIT 12f -> CATCH_PULL 13f -> THROW 24f -> SHOULDER_START 15f -> SHOULDER_WAIT 88f -> THROW_F_F 31f -> WAIT
//
// Each of those answers a playtest report on its own. A whip that never grabbed reads
// `SPECIAL_S 64f -> SPECIAL_S_END 20f -> WAIT` -- no THROW, so the catch never fired. A carry
// that froze would have shown the same status repeating. A Revenge that spent its boost on its
// own fire shows SPECIAL_LW_HIT present, which tells you the retaliation ran and the bug is in
// what it did rather than in whether it happened.
//
// THE POINT IS THE SHAPE, NOT THE DETAIL. The detail is already in the per-frame lines; this is
// the index into them.
use smash::{app::{lua_bind::*, BattleObjectModuleAccessor}, lib::lua_const::*};
use super::sink;

/// Longest trail worth keeping. The longest real move here is the cargo carry at eight statuses;
/// anything longer is a bug and gets reported as a truncated trail, which is itself a signal.
const MAX: usize = 14;

static mut TRAIL: [[i32; MAX]; 8] = [[0; MAX]; 8];
static mut FRAMES: [[u32; MAX]; 8] = [[0; MAX]; 8];
static mut LEN: [usize; 8] = [0; 8];
static mut LAST: [i32; 8] = [-1; 8];
static mut OVERFLOWED: [bool; 8] = [false; 8];

/// Statuses that mean "not in a move" -- the trail is flushed when one is reached.
///
/// Deliberately a short list of the ordinary resting states rather than "anything below 0x1DC".
/// A move that ends by being hit should flush too, and DAMAGE is how that reads.
unsafe fn is_idle(s: i32) -> bool {
    s == *FIGHTER_STATUS_KIND_WAIT
        || s == *FIGHTER_STATUS_KIND_FALL
        || s == *FIGHTER_STATUS_KIND_LANDING
        || s == *FIGHTER_STATUS_KIND_DAMAGE
        || s == *FIGHTER_STATUS_KIND_DAMAGE_AIR
        || s == *FIGHTER_STATUS_KIND_DAMAGE_FLY
        || s == *FIGHTER_STATUS_KIND_WALK
        || s == *FIGHTER_STATUS_KIND_DASH
        || s == *FIGHTER_STATUS_KIND_SQUAT_WAIT
        || s == *FIGHTER_STATUS_KIND_GUARD
}

/// Which move a status belongs to, for the label. None means it is not one this moveset owns.
unsafe fn family(s: i32) -> Option<&'static str> {
    use crate::specials::status::*;
    if s == *FIGHTER_STATUS_KIND_SPECIAL_S
        || (RAT_STATUS_KIND_SPECIAL_S_SHOULDER..=RAT_STATUS_KIND_SPECIAL_S_FAILURE).contains(&s)
    {
        return Some("side-B whip");
    }
    if s == *FIGHTER_STATUS_KIND_SPECIAL_LW || s == RAT_STATUS_KIND_SPECIAL_LW_HIT {
        return Some("down-B revenge");
    }
    if s == *FIGHTER_STATUS_KIND_SPECIAL_HI
        || s == RAT_STATUS_KIND_SPECIAL_HI_CLOSE
        || s == RAT_STATUS_KIND_SPECIAL_HI_OPEN
    {
        return Some("up-B");
    }
    if (RAT_STATUS_KIND_SHOULDER_START..=RAT_STATUS_KIND_THROW_F_LW).contains(&s) {
        return Some("cargo carry");
    }
    if s == *FIGHTER_STATUS_KIND_CATCH
        || s == *FIGHTER_STATUS_KIND_CATCH_WAIT
        || s == *FIGHTER_STATUS_KIND_CATCH_PULL
        || s == *FIGHTER_STATUS_KIND_CATCH_ATTACK
        || s == *FIGHTER_STATUS_KIND_THROW
    {
        return Some("grab");
    }
    None
}

/// Call once per frame with the current status.
pub unsafe fn tick(boma: *mut BattleObjectModuleAccessor, entry: usize, status: i32) {
    if entry >= 8 {
        return;
    }
    let changed = status != LAST[entry];
    LAST[entry] = status;

    if LEN[entry] > 0 {
        // Time is charged to the status we were in, so a frozen or stuck state shows a large
        // number next to its own name rather than being invisible.
        let i = LEN[entry] - 1;
        FRAMES[entry][i] = FRAMES[entry][i].saturating_add(1);
    }

    if !changed {
        return;
    }

    // Reaching an ordinary resting state ends the move.
    if is_idle(status) {
        flush(entry, status);
        return;
    }
    if family(status).is_none() && LEN[entry] == 0 {
        // Not a move of ours and not already recording. Nothing to do.
        return;
    }
    if LEN[entry] >= MAX {
        OVERFLOWED[entry] = true;
        return;
    }
    TRAIL[entry][LEN[entry]] = status;
    FRAMES[entry][LEN[entry]] = 0;
    LEN[entry] += 1;
    let _ = boma;
}

/// Emit the trail and clear it.
unsafe fn flush(entry: usize, ended_in: i32) {
    if LEN[entry] == 0 {
        return;
    }
    let label = (0..LEN[entry]).filter_map(|i| family(TRAIL[entry][i])).next().unwrap_or("move");
    let mut line = format!("[rat] MOVE p{entry} {label}: ");
    for i in 0..LEN[entry] {
        line.push_str(&format!(
            "{} {}f -> ",
            super::probe::st(TRAIL[entry][i]),
            FRAMES[entry][i]
        ));
    }
    line.push_str(&super::probe::st(ended_in));
    if OVERFLOWED[entry] {
        line.push_str(&format!(
            "  (TRUNCATED at {MAX} statuses -- a move this long is itself a bug)"
        ));
    }
    // An event rather than a buffered line: this is the summary a reader looks for first, and it
    // should survive whatever happens next.
    sink::log(line);
    LEN[entry] = 0;
    OVERFLOWED[entry] = false;
}
