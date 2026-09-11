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
use smash::app::{lua_bind::*, utility, BattleObjectModuleAccessor};
use smash::lib::lua_const::*;

/// `hash40("param_special_s")`, the param OBJECT the victim's status asks for.
const PARAM_SPECIAL_S: u64 = 0x0f_ea97_fe73;

/// The int keys, and Incineroar's values from `extracted/fighter/gaogaen/param/vl.prc`.
///
/// Each hash is an immediate lifted straight out of `lua2cpp_common.nro` -- the `mov`/`movk`
/// triple loaded into x1 before a `WorkModule::get_param_int_impl` call inside one of the
/// `SwingGaogaen*` statuses -- so the pairing of key to GETTER is the game's, not a guess about
/// whether a prc field is an int or a float.
static INTS: &[(u64, i32, &str)] = &[
    // How long the victim is held and mashing. The one that matters most.
    (0x1d_f2c4_02be, 85, "special_s_catch_clatter_frame"),
    (0x21_87e5_2777, 5, "special_s_catch_clatter_dec_frame"),
    (0x2c_271c_b634, 0, "special_s_catch_clatter_damage_mul_frame_add"),
    // How long after release they cannot be grabbed again.
    (0x1f_d42d_6134, 60, "special_s_invalid_capture_frame"),
    // The outbound leg of the throw, with ropes and without.
    (0x25_21e0_070e, 30, "special_s_through_target_cancel_frame"),
    (0x2d_dbde_d58f, 30, "special_s_through_target_cancel_frame_no_rope"),
    // Where their flight animation is pinned as they leave and as they come back.
    (0x29_1266_e331, 8, "special_s_through_target_motion_frame_fix"),
    (0x30_fccd_50a3, 8, "special_s_through_target_return_motion_frame_fix"),
];

/// The float keys, same provenance.
static FLOATS: &[(u64, f32, &str)] = &[
    (0x1e_c20e_1e00, 11.2, "special_s_through_target_speed"),
    (0x25_b2ed_14e5, 2.8, "special_s_through_target_return_speed"),
    (0x26_7f28_d95d, 1.6, "special_s_through_target_speed_no_rope"),
    (0x28_dd07_f91b, 1.6, "special_s_through_target_speed_over_rope"),
];

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

pub fn install() {
    skyline::install_hooks!(get_param_int_hook, get_param_float_hook);
}

// Two things that must stay true, checked at compile time rather than trusted: the whip module's
// hardcoded copies of these numbers and this table's are the same numbers. They are read from the
// same prc by tools/check-srcparams.sh, but that checks each against the prc, not against each
// other -- and the pair that matters is the pair the ENGINE and OUR CODE use for the same throw.
const _: () = {
    assert!(INTS[0].1 == crate::specials::specials::CATCH_CLATTER_FRAME);
    assert!(INTS[4].1 == crate::specials::specials::THROUGH_CANCEL_FRAME);
};
