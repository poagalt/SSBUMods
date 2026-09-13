// Answers the engine when it asks OUR Pikachu for a param he does not have.
//
// THIS IS THE MISSING HALF OF ALOLAN WHIP, AND IT IS NOT CODE -- IT IS DATA.
//
// The victim's side of the whip is common engine code, which is the whole reason the move is
// portable: `FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED` and its siblings live in
// `lua2cpp_common.nro` and run for whoever gets grabbed, not for Incineroar. What that code does
// NOT do is carry Incineroar's numbers. It reads them, every frame, off THE GRABBER:
//
//     status_SwingGaogaenCatched:
//       LinkModule::get_parent_id            -> who grabbed me
//       sv_battle_object::module_accessor    -> their accessor
//       WorkModule::get_param_int (that accessor, param_special_s, special_s_catch_clatter_frame)
//
// Twelve reads, all of them through the parent's accessor, disassembled out of the NRO and listed
// below. When the grabber is Pikachu, `param_special_s` IS THERE -- it is Skull Bash's, charge
// rates and flick thresholds -- and it contains none of those keys, so every one of those reads
// returns 0.
//
// THAT IS THE BUG THE PLAYTESTS KEPT DESCRIBING. `special_s_catch_clatter_frame` is 85: the
// number of frames the victim is pinned, mashing, before the grab releases. Read as 0, the
// clatter is over before it starts and the engine hands them straight back their own controls --
// "it appears the opponent can still attack while the grab is happening". No amount of holding
// them in place from our side fixes that, because it was never our side that let them go.
//
// WHY A HOOK AND NOT A `vl.prc`. Shipping a merged vl.prc is the obvious answer and it was tried:
// `tools/gen-vlprc.sh` replaced Pikachu's `param_special_s` with Incineroar's, and Pikachu
// CRASHED ON FIGHTER LOAD -- a null dereference reached through `smashline + 0x1769c
// (load_fighter_effects + 0x48)`, which is smashline's inline hook handing the game's own effect
// loader an x0 of 0. The generator is still there, still disabled, with that crash written down.
// A hook needs no file, changes no layout, cannot be loaded at the wrong moment, and is off for
// every object that is not a Pikachu asking for that one object.
//
// SCOPE, deliberately narrow. Only `param_special_s`, only the twelve keys the disassembly proves
// are read, only on a Pikachu. Incineroar's other thirty-eight `param_special_s` entries are read
// by HIS native code keyed on HIS fighter kind, which never runs here; this port hardcodes those
// numbers in `src/specials/specials/mod.rs` and `tools/check-srcparams.sh` already checks them
// against the same prc. Adding them would create a second copy of a number nothing reads.
//
// Pikachu's own keys are untouched -- every one of his is `special_s_*_` with a trailing
// underscore and none collides with Incineroar's -- and any key not in these tables falls through
// to the game.
mod table;

use smash::app::{lua_bind::*, utility, BattleObjectModuleAccessor};
use smash::lib::lua_const::*;

/// `hash40("param_special_s")`, the param OBJECT the victim's status asks for.
const PARAM_SPECIAL_S: u64 = 0x0f_ea97_fe73;

/// THE WHOLE OBJECT, GENERATED. `table.rs` is written by tools/gen-paramprovider.sh straight from
/// `extracted/fighter/gaogaen/param/vl.prc`, and the int/float split is the prc's own.
///
/// IT USED TO BE TWELVE KEYS, TYPED BY HAND -- the twelve the VICTIM's status reads, with a
/// comment arguing that Incineroar's other thirty-eight are read by HIS native code, which never
/// runs for a Pikachu, so providing them would be a second copy of a number nothing reads.
///
/// That stopped being true when the whip became a transcription. `FUN_710001d4b0` -- the kinetic
/// setup all six of his side-B statuses call -- reads eight more, and a transcription of it
/// running on a Pikachu reads them off PIKACHU: special_s_start_speed_mul,
/// special_air_s_start_speed_mul, special_air_s_catch_accel_y, special_air_s_catch_speed_y_max,
/// special_s_catch_speed_x_mul, special_s_catch_brake_x and both air catch equivalents. Every one
/// would have come back 0, which in a kinetic setup is a move that does not move.
///
/// Generated rather than transcribed because fifty hand-typed floats is fifty chances to typo a
/// value that then looks like a physics bug.
use table::{FLOATS, INTS};

/// Is this accessor a Pikachu?
///
/// Called only after the object hash has already matched, which is one compare and is false for
/// every param read in the game except Skull Bash's and Incineroar's own. That ordering is what
/// keeps a hook on one of the hottest functions in the engine from costing anything.
unsafe fn is_pikachu(boma: *mut BattleObjectModuleAccessor) -> bool {
    !boma.is_null() && utility::get_kind(&mut *boma) == *FIGHTER_KIND_PIKACHU
}

#[skyline::hook(replace = WorkModule::get_param_int)]
unsafe fn get_param_int_hook(
    boma: *mut BattleObjectModuleAccessor,
    object: u64,
    key: u64,
) -> i32 {
    if object == PARAM_SPECIAL_S && is_pikachu(boma) {
        if let Some(i) = INTS.iter().position(|(k, _, _)| *k == key) {
            let (_, value, name) = INTS[i];
            announce(std::ptr::addr_of_mut!(SEEN_INT), i, name, value as f32);
            return value;
        }
        // A FLOAT-TYPED PARAM ASKED FOR AS AN INT still has an answer. See the note on the float
        // hook: the prc's type and the caller's accessor are independent, and the engine coerces.
        if let Some(i) = FLOATS.iter().position(|(k, _, _)| *k == key) {
            let (_, value, name) = FLOATS[i];
            announce(std::ptr::addr_of_mut!(SEEN_FLOAT), i, name, value);
            return value as i32;
        }
    }
    original!()(boma, object, key)
}

#[skyline::hook(replace = WorkModule::get_param_float)]
unsafe fn get_param_float_hook(
    boma: *mut BattleObjectModuleAccessor,
    object: u64,
    key: u64,
) -> f32 {
    if object == PARAM_SPECIAL_S && is_pikachu(boma) {
        if let Some(i) = FLOATS.iter().position(|(k, _, _)| *k == key) {
            let (_, value, name) = FLOATS[i];
            announce(std::ptr::addr_of_mut!(SEEN_FLOAT), i, name, value);
            return value;
        }
        // THE PRC'S TYPE AND THE CALLER'S ACCESSOR ARE INDEPENDENT, AND MISSING THAT COST THE
        // WHOLE SECOND HALF OF THE WHIP.
        //
        // `special_s_lariat_offset_x1` is stored in the prc as an INT (-24), so the generator put
        // it in `INTS` -- but Incineroar's own status reads it with `get_param_float`:
        //
        //     call WorkModule__get_param_float_impl  ["param_special_s", "special_s_lariat_offset_x1"]
        //
        // The engine coerces; this hook did not. It searched only `FLOATS`, missed the key, and
        // fell through to the real engine, where a Pikachu has no `param_special_s` at all -- so
        // the answer was 0. The playtest diagnostic said it in one line:
        //
        //     WHIP p0 f50 area: off=-18.29 in=false lariat=0 end=0 fail=0
        //
        // All three area thresholds zero instead of -24/-10/-6, so the lariat window could never
        // be entered no matter where the victim flew -- and `off` shows he flew perfectly, +16
        // held in front then -10, -26, -42 behind. The same bug zeroed
        // `special_s_lariat_motion_rate_min`/`_max`, which would have given the lariat a motion
        // rate of 0: a frozen animation.
        //
        // Fixed HERE rather than at the five call sites, because the call sites are right. They
        // are transcriptions of a source that uses `get_param_float`, and changing them to match
        // the prc's storage type would make the port disagree with the game it copies. The engine
        // answers either accessor for either storage type, so this provider must too.
        if let Some(i) = INTS.iter().position(|(k, _, _)| *k == key) {
            let (_, value, name) = INTS[i];
            announce(std::ptr::addr_of_mut!(SEEN_INT), i, name, value as f32);
            return value as f32;
        }
    }
    original!()(boma, object, key)
}

/// Says ONCE per key that the engine came asking, then never again.
///
/// A per-frame line from inside a status the victim runs every frame would bury the log, and a
/// silent hook would leave the most important question about this whole module unanswerable: does
/// the engine actually read these, or is the whip's victim side never reaching that code at all?
/// One line per key answers it and costs twelve lines total.
unsafe fn announce(seen: *mut u16, i: usize, name: &str, value: f32) {
    let bit = 1u16 << i;
    if *seen & bit != 0 {
        return;
    }
    *seen |= bit;
    crate::telemetry::sink::diag(format!(
        "[rat] PARAM the engine asked Pikachu for {name} -- answered {value} (Incineroar's; \
         his own param_special_s has no such key and would read 0)"
    ));
}

/// One bit per table entry, so a key announces itself once and then costs a mask test. A raw
/// pointer rather than `&mut`, because a reference to a `static mut` is a warning the rest of
/// this plugin avoids by only ever indexing them.
static mut SEEN_INT: u16 = 0;
static mut SEEN_FLOAT: u16 = 0;

/// `FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLAG_SPECIAL_S_GENERATED_ROPE`, which the VICTIM reads OFF US.
///
/// THE VICTIM'S OWN NATIVE STATUS ASKS THE CAPTOR WHETHER A ROPE EXISTS, and answers it for itself.
/// `L2CFighterCommon::status_SwingGaogaenCatched` in `lua2cpp_common` (0x710028a510) resolves the
/// captor through `LinkModule::get_parent_id(LINK_NO_CAPTURE)` and then reads, off the captor:
///
///     is_flag(FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLAG_SPECIAL_S_GENERATED_ROPE)
///       if OFF  -> special_s_through_target_cancel_frame_no_rope
///                  special_s_through_target_speed_no_rope
///       if ON   -> special_s_through_target_cancel_frame / _speed / _speed_over_rope
///                  and get_float(FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLOAT_ROPE_X)
///
/// So the game HAS a first-class no-rope path and the victim selects it automatically -- which is
/// exactly the situation ADR 0004 puts this port in permanently. It just has to answer honestly.
///
/// IT CANNOT ANSWER BY WRITING THE FLAG. That id is 0x200000EA and Pikachu's
/// FIGHTER_PIKACHU_INSTANCE_WORK_ID_FLAG_TERM is 0x200000E8, so it is TWO PAST THE END of his
/// instance-flag array -- not an alias of some other flag, genuinely out of bounds. `off_flag`
/// there would be an out-of-bounds write, and leaving it alone means the victim's `is_flag` reads
/// whatever sits past the array and may well decide a rope exists. That is the ADR 0004 crash
/// arriving from the one direction the port cannot reach: the victim's code, reading our state.
///
/// So the read is intercepted instead, which is the same move `get_param_*` above already makes
/// and for the same reason: the engine asks a question about a Pikachu that only this plugin knows
/// the answer to. `false` is not a guess -- this port ships no rope, by decision.
const GENERATED_ROPE: i32 = 0x2000_00EA;

/// `FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLOAT_ROPE_X`, the other half of the same question.
///
/// STATUS-scoped (the 0x1000000 prefix), index 7 into the float array of whatever status the
/// object is in -- which for us is a smashline-created status whose array we do not size. So this
/// is answered rather than written, for the same reason `GENERATED_ROPE` is: the reader is the
/// VICTIM, reaching through `LinkModule::get_parent_id(LINK_NO_CAPTURE)` onto our accessor, and
/// the only thing that knows the answer is this plugin.
const SPECIAL_S_FLOAT_ROPE_X: i32 = 0x0100_0007;

/// `WorkModule::get_int(boma, FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID)` without the import cycle.
unsafe fn entry_of(boma: *mut BattleObjectModuleAccessor) -> usize {
    let e = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if e >= 8 { 0 } else { e }
}

/// ANSWERING "YES, THERE IS A ROPE" -- WHICH REVERSES WHAT THIS HOOK USED TO SAY, AND THE REASON
/// IS THAT "NO" WAS HONEST ABOUT THE ARTICLE AND WRONG ABOUT THE MOVE.
///
/// The note below argued `false` was "not a guess -- this port ships no rope, by decision", and
/// that the game's no-rope path is "exactly the situation ADR 0004 puts this port in
/// permanently". Both sentences are true and the conclusion was still wrong, because of what the
/// two branches actually differ by. From the victim's `status_SwingGaogaenCatched`:
///
///     rope    -> special_s_through_target_speed          11.2
///                special_s_through_target_cancel_frame   30
///                special_s_through_target_speed_over_rope 1.6
///                get_float(SPECIAL_S_FLOAT_ROPE_X)       <- where to stop
///     no rope -> special_s_through_target_speed_no_rope  1.6
///                special_s_through_target_cancel_frame_no_rope 30
///
/// The no-rope branch is not "the same throw without a visual". It is a SEVEN TIMES SLOWER throw
/// that stops on a frame count instead of at a place, because with no rope there is nowhere to
/// bounce and the move is not supposed to come back. Taking it and then hand-triggering a rebound
/// at the rope's distance is two halves of two different moves: the playtest called it "the travel
/// distance/time isn't right", and the log shows why -- the victim crawling from +14.74 at f30 to
/// -24.69 at f55 instead of being flung there in five frames.
///
/// WHAT THE ROPE BRANCH ACTUALLY NEEDS IS A NUMBER, NOT AN OBJECT. Every one of those four reads
/// is a param or a float off the captor; none of them touches `GENERATE_ARTICLE_ROPE`. The
/// article's own jobs -- the visual, and raising the area flags from a real collision -- are the
/// two things this port substitutes for and documents as substituted. So this says yes, and
/// supplies the coordinate, and ADR 0004 is untouched: no victim is handed to code that
/// dereferences an article we do not ship.
///
/// It says yes only once the whip has somewhere to put the rope, so a stray read outside the move
/// still gets the honest answer.
#[skyline::hook(replace = WorkModule::is_flag)]
unsafe fn is_flag_hook(boma: *mut BattleObjectModuleAccessor, id: i32) -> bool {
    if id == GENERATED_ROPE && is_pikachu(boma) {
        return crate::specials::whip::state::rope_x_set(entry_of(boma));
    }
    original!()(boma, id)
}

#[skyline::hook(replace = WorkModule::get_float)]
unsafe fn get_float_hook(boma: *mut BattleObjectModuleAccessor, id: i32) -> f32 {
    if id == SPECIAL_S_FLOAT_ROPE_X && is_pikachu(boma) {
        let entry = entry_of(boma);
        if crate::specials::whip::state::rope_x_set(entry) {
            let x = crate::specials::whip::state::rope_x(entry);
            announce_once(std::ptr::addr_of_mut!(SEEN_ROPE_X), "rope_x", x);
            return x;
        }
    }
    original!()(boma, id)
}

static mut SEEN_ROPE_X: bool = false;

unsafe fn announce_once(seen: *mut bool, name: &str, value: f32) {
    if *seen {
        return;
    }
    *seen = true;
    crate::telemetry::sink::diag(format!(
        "[rat] PARAM the engine asked Pikachu for {name} -- answered {value} (the whip's rope \
         line; this is the read that decides where the victim stops)"
    ));
}

pub fn install() {
    skyline::install_hooks!(
        get_param_int_hook,
        get_param_float_hook,
        is_flag_hook,
        get_float_hook
    );
}

// Two things that must stay true, checked at compile time rather than trusted: the whip module's
// hardcoded copies of these numbers and this table's are the same numbers. They are read from the
// same prc by tools/check-srcparams.sh, but that checks each against the prc, not against each
// other -- and the pair that matters is the pair the ENGINE and OUR CODE use for the same throw.
// The cross-check the hand-typed table needed is gone: the values now come from the prc by
// construction, and tools/check.sh step 3e2 verifies every generated row against that same prc,
// which checks that the GENERATOR RAN rather than that someone typed carefully.
