// Incineroar's Alolan Whip (side-B): a travelling command grab that Irish-whips the victim into
// the wrestling ring ropes and hits them on the rebound.
//
// WHAT THE MOVE ACTUALLY IS, checked against SmashWiki rather than guessed. Incineroar dashes
// forward with both arms out. On a grab, a set of ropes rises up behind him at about the distance
// the dash covered, he whips the opponent into them, and they rebound back towards him. What
// happens next is decided by WHETHER AND WHEN an attack or special button is pressed:
//
//   pressed early, any time after the grab      -> BACK BODY DROP, launches them vertically
//   pressed just as they collide with him       -> LARIAT, launches them horizontally
//   never pressed                               -> FAILURE, both take 4%
//
// This replaced an earlier reading in which the follow-up was chosen from the control STICK and
// the whiffed lunge was the thing that dealt the 4%. Both were wrong, and the mapping is not a
// guess any more: it falls straight out of Incineroar's own params, below.
//
// TUNING FROM fighter/gaogaen/param/vl.prc, param_special_s[0], read with tools/prc-tool prcdump:
//   special_s_start_speed_mul                 1.3    the lunge, as a multiplier on MOTION energy
//   special_s_catch_speed_x_mul               0.2    momentum kept once the grab connects
//   special_s_catch_brake_x                   0.25
//   special_s_catch_clatter_frame             85     how long the victim can mash before escaping
//   special_s_through_target_speed            11.2   how fast the victim is whipped at the ropes
//   special_s_through_target_return_speed     2.8    how fast they come back
//   special_s_through_target_cancel_frame     30     frames the outbound leg lasts
//   special_s_through_target_speed_no_rope    1.6    the same throw with no ropes to bounce off
//   special_s_fail_damage                     4      what a failed follow-up costs him
//   special_s_lariat_offset_x1/x2            -24     LARIAT detection region, behind him
//   special_s_lariat_end_offset_x1/x2        -10     the tight, late part of that region
//   special_s_failure_offset_x1/x2            -6     where the rebound counts as a collision
//
// Those last three are the whole follow-up mapping in numbers: three nested regions behind
// Incineroar at 24, 10 and 6 units, matched by his FLAG_LARIAT_AREA, FLAG_LARIAT_END_AREA and
// FLAG_FAILURE_AREA. Which region means which is settled by one more number:
// game_specialslariat writes special_s_lariat_hit_frame = 9 and puts its arml hitbox out on frame
// 9. A victim returning at through_target_return_speed 2.8 covers the 24 units to Incineroar in
// 24 / 2.8 = 8.6 frames. So the 24-unit region is exactly "a lariat started now connects", and 10
// is where it is too late to start one. That is the "pressed just as they collide" window, and it
// is 24 units wide rather than a single frame.
//
// So: press with the victim between 10 and 24 units out -> LARIAT. Press earlier than that
// (including while they are still being held, or still flying outward) -> BACK BODY DROP. No
// press by the time they reach 6 units -> FAILURE. The outcome is decided by measured distance,
// not by an invented input scheme.
//
// FIVE STATUSES, NOT ONE WITH A PHASE VARIABLE. Incineroar has SPECIAL_S_THROW, _SHOULDER,
// _LARIAT, _FAILURE and _END as real status kinds (his own extras, 0x1E1-0x1E5). The earlier port
// crammed all of them into FIGHTER_STATUS_KIND_SPECIAL_S with a phase variable and re-entered its
// own Main function recursively, because adding status kinds looked impossible. It is not --
// smashline extends the status table, see ../status.rs -- so each follow-up now gets its own
// status, which is both the source's shape and the reason the recursive re-entry is gone.
//
// THE ROPES, AND WHY THE TRIP IS DRIVEN HERE RATHER THAN BY THE ENGINE.
//
// The visible ropes are Incineroar's ROPE article (FIGHTER_GAOGAEN_GENERATE_ARTICLE_ROPE,
// WEAPON_KIND_GAOGAEN_ROPE 0x1BA) and are not reproduced. Cloning an article is the machinery that
// crashes Ptooie on fighter load, and the documented fix for that crash -- merging `param_spikeball`
// into Pikachu's vl.prc -- is now known to be a cause rather than a cure: shipping ANY modified
// vl.prc null-dereferences in the same smashline function, `load_fighter_effects`. So that route is
// closed until the crash is understood, not merely worked around.
//
// The victim's statuses ARE all engine-global (SWING_GAOGAEN_THROWN 0x177, _ATTACH_ROPE 0x178,
// _RETURN 0x179) and Pikachu ships `gaogaen_special_s_thrown_big` for being on the receiving end.
// But those statuses read their trajectory off the GRABBER's `param_special_s` -- and Pikachu's is
// Skull Bash's, with no `special_s_through_target_speed` in it, so every lookup returns 0 and the
// whipped opponent goes nowhere. Replacing that param object is exactly what crashed on load.
//
// So the trajectory is driven from here instead, with Incineroar's own numbers, applied to the
// victim's posture directly. That is more faithful than it sounds: the values are his, the frame
// counts are his, and driving position rather than velocity means the arc does not vary with the
// victim's weight or their own physics -- which is what a scripted throw should do.
//
// What is genuinely lost: the ropes are invisible, and `_speed_over_rope` (the case where the
// victim clears the ropes entirely) is not modelled.
use super::status::*;
use {
    smash::{
        lua2cpp::*,
        phx::*,
        app::{sv_animcmd::*, lua_bind::*, *},
        lib::{lua_const::*, L2CValue, L2CAgent},
        hash40
    },
    smash_script::*,
    smashline::{*, Priority::*}
};

// param_special_s[0]. Kept as named constants rather than read at runtime because Pikachu's
// vl.prc has no param_special_s object to read them out of.
const START_SPEED_MUL: f32 = 1.3;
const CATCH_CLATTER_FRAME: i32 = 85;
const FAIL_DAMAGE: f32 = 4.0;
const THROUGH_CANCEL_FRAME: i32 = 30;
/// `special_s_through_target_speed` -- how fast the victim is whipped away.
const THROUGH_SPEED: f32 = 11.2;
/// `special_s_through_target_return_speed` -- how fast they come back off the ropes.
const THROUGH_RETURN_SPEED: f32 = 2.8;
/// Far edge of the LARIAT window, `special_s_lariat_offset_x`. A lariat started with the victim
/// here connects: 24 units at return_speed 2.8 is 8.6 frames, and the lariat's hitbox comes out on
/// frame 9.
const LARIAT_X: f32 = 24.0;
/// Near edge, `special_s_lariat_end_offset_x`. Inside this it is too late to start one.
const LARIAT_END_X: f32 = 10.0;
/// Frames after the whip before the lariat band is believed. The outbound leg runs at
/// through_target_speed 11.2, so the victim clears 24 units in about two frames; without this the
/// band would match on the way out as well as on the way back.
const RETURN_GUARD_FRAMES: i32 = 3;
/// `special_s_failure_offset_x` -- the rebound has arrived.
const FAILURE_X: f32 = 6.0;

/// Real frames into game_specialsthrow at which FLAG_THROW_ROPE fires. The ACMD raises it on
/// script frame 37, and FT_MOTION_RATE runs the script at 0.75 from frame 1 and 0.5 from frame 20
/// -- rate being game frames per motion frame -- so it lands about 24 real frames in. Used only as
/// a backstop; the flag itself is what this reads, now that deforeign keeps it.
const THROW_ROPE_FALLBACK_FRAME: i32 = 24;

// WHAT THIS PORT DOES NOT REPRODUCE, from `gaogaen param_special_s`. Enumerated because
// tools/check-srcparams.sh asks and because a gap nobody wrote down is a gap nobody fixes.
//
// FIRST, THREE THAT TURN OUT NOT TO BE GAPS AT ALL. `special_air_s_start_speed_mul` is 1.3,
// `special_air_s_catch_speed_x_mul` is 0.2 and `special_air_s_catch_brake_x` is 0.25 -- identical
// to their grounded counterparts, which this module already uses. The air whip sharing START_SPEED_MUL
// with the grounded one is therefore correct rather than a shortcut, and worth knowing before
// anyone "fixes" it.
//
// NOT MODELLED: special_air_s_catch_speed_y_max
//   0.05, and the only air param with no grounded twin: a cap on vertical drift while holding
//   someone in the air. Real, small, and cheap to add once the air whip has been seen to work.
//
// NOT MODELLED: special_s_lariat_node special_s_lariat_size special_s_lariat_offset_y1
// NOT MODELLED: special_s_lariat_offset_x2 special_s_lariat_offset_y2
// NOT MODELLED: special_s_lariat_end_node special_s_lariat_end_size
// NOT MODELLED: special_s_lariat_end_offset_y1 special_s_lariat_end_offset_x2
// NOT MODELLED: special_s_lariat_end_offset_y2
// NOT MODELLED: special_s_failure_node special_s_failure_size
// NOT MODELLED: special_s_failure_offset_y1 special_s_failure_offset_x2
// NOT MODELLED: special_s_failure_offset_y2
//   Each band is really a CAPSULE on `top` -- x1,y1 to x2,y2 with a radius -- and this module
//   reads only x1, as a one-dimensional gate on the gap between Pikachu and the victim. The
//   vertical span (y 12 down to 2, radius 2) is what makes it a region rather than a line. The
//   simplification holds because `drive_victim` moves the victim along x only, so their y never
//   leaves the band; it stops holding the moment the airborne whip lets them separate vertically.
//
// NOT MODELLED: special_s_attack_after_control_speed_x_mul special_s_attack_after_control_speed_x_stable
//   0.9 and 0.7: reduced air control after the move, which is part of how committal it feels.
//
// NOT MODELLED: special_s_invalid_capture_frame
//   60 frames during which the victim cannot be grabbed again. Anti-infinite protection, and the
//   one item on this list whose absence is a balance problem rather than a fidelity one.
//
// NOT MODELLED: special_s_lariat_motion_rate_min special_s_lariat_motion_rate_max
//   The lariat plays between 1x and 2x speed. What selects the rate is Incineroar's C++, not any
//   ACMD, so the endpoints are known and the input is not.
//
// NOT MODELLED: special_s_through_target_motion_frame_fix special_s_through_target_return_motion_frame_fix
// NOT MODELLED: special_s_catch_clatter_damage_mul_frame_add special_s_catch_clatter_dec_frame
// NOT MODELLED: special_s_through_target_cancel_frame_no_rope special_s_through_target_speed_over_rope
//   Motion-frame fixes during the drag, clatter decay, and the no-rope and over-rope variants for
//   stage geometry this port does not distinguish.
//
// NOT MODELLED: critical_frame critical_slow_rate critical_zoom_rate critical_zoom_in_frame
// NOT MODELLED: critical_camera_offset_x critical_camera_offset_y critical_1_on_1
//   The KO cinematic. Presentation, deferred with the rest of the visual layer.

/// The victim, captured when the grab connects. `CatchModule::capture_object_id` stops answering
/// once they leave the CATCHED status, and the follow-up decision needs their position for the
/// whole rebound, so it is cached rather than re-read.
static mut VICTIM: [u32; 8] = [0; 8];
/// Frames the victim has been held, against CATCH_CLATTER_FRAME.
static mut HELD_FOR: [i32; 8] = [0; 8];
/// Frames since FLAG_THROW_ROPE. -1 while the whip has not happened yet.
static mut SINCE_THROW: [i32; 8] = [-1; 8];
/// Whether the lunge pull-in hitbox has already been seen to connect this attempt.
///
/// `AttackModule::is_attack_occur` is sticky once an attack lands, so it is true for the rest of
/// the move. Only the EDGE is a signal -- the frame it first goes true is the frame an opponent
/// was demonstrably inside the region the catch boxes are about to occupy, and the frame before
/// the pull-ins knockback moves them out of it.
static mut PULLIN_SEEN: [bool; 8] = [false; 8];

/// Every live fighter that is not us, as `(battle object id, module accessor)`.
///
/// THE ID ARITHMETIC IS MEASURED. Round 4's telemetry logged the object id the engine itself
/// handed back during Pikachu's ordinary grab: 0x10000, for the fighter at entry 1. A fighter's
/// battle object id is `entry << 16`. This was the one thing blocking any victim-side check, and
/// three previous rounds declined to guess it -- correctly, since 0x5000000n was the obvious
/// guess from the invalid sentinel and it is wrong.
///
/// `battle_object_id` on our own accessor is what we compare against, rather than
/// `ENTRY_ID << 16` again: the engine wrote it, so a capture link that names us is recognised
/// even if the entry arithmetic is ever wrong.
unsafe fn other_fighters(
    boma: *mut BattleObjectModuleAccessor,
) -> Vec<(u32, *mut BattleObjectModuleAccessor)> {
    crate::victim::others((*boma).battle_object_id)
}

/// The fighter our command grab just caught, and how we know.
///
/// ASK THE VICTIM, BECAUSE INCINEROAR NEVER ASKS ANYTHING.
///
/// This round finally read the source instead of reasoning about it. `tools/disasm` resolves the
/// lua2cpp NRO's PLT against its own .dynsym, so every call in the two helpers the public dump
/// left unnamed now prints with a name -- and the answer is that neither is about catching.
/// FUN_710001ff80 is the air-control block: `WorkModule::is_flag`, then `reset_energy`,
/// `controller_set_accel_x_mul`, `controller_set_accel_x_add`, `set_stable_speed`. FUN_710001d4b0
/// is the ground/air motion switch.
///
/// More conclusive still: `disasm --sym catch` over the whole gaogaen NRO finds exactly five
/// symbols, and they are `sv_animcmd::CATCH`, `FT_CATCH_STOP`,
/// `AttackModule__set_catch_only_all_impl` and two expression rumbles. There is no
/// `CatchModule__is_catch_impl` import anywhere in the module, and `grep -ri catch` over the
/// whole decompiled status tree returns nothing. **Incineroar's status scripts do not detect the
/// catch at all.** `SpecialS/main.c` clears
/// `FIGHTER_GAOGAEN_INSTANCE_WORK_ID_INT_BATTLE_OBJECT_ID_SWING_THROWN_FIGHTER` to
/// BATTLE_OBJECT_ID_INVALID on entry and five later statuses read it back, but nothing in the NRO
/// ever writes it. The engine does, in native code, and it transitions him into SPECIAL_S_THROW
/// itself.
///
/// That native path keys on the fighter being Gaogaen, so it will never fire for Pikachu. Doing
/// it here is not a workaround for a missing signal -- it is the only place it can be done.
///
/// SO ASK THE TWO QUESTIONS THE ENGINE CAN ANSWER, on the victim rather than on us:
///
///   `CaptureModule::is_capture` + `catch_object_id` -- the engine's own capture link, naming
///   who holds them. This is the authoritative one and it is new this round; every previous
///   attempt asked the CATCHER (`CatchModule::is_catch`, `capture_object_id`, `GrabModule::
///   is_grab`), and those are about ordinary CAPTURE bookkeeping that a command grab need not
///   touch.
///
///   the caught status itself -- the ACMD names the victim's destination,
///   `macros::CATCH(..., SWING_GAOGAEN_CATCHED, ...)`, an engine-common status at 0x176. If the
///   collision connected and the engine moved them, somebody is in it.
///
/// Either is sufficient. The reason both are read is that they fail differently: the link
/// without the status means the engine caught them but left them in their own status, and the
/// status without the link means it moved them without recording a master -- and the port has to
/// cope with either, since it drives the victim's position itself from here on.
unsafe fn find_caught_victim(boma: *mut BattleObjectModuleAccessor) -> Option<(u32, &'static str)> {
    let me = (*boma).battle_object_id;
    for (id, vboma) in other_fighters(boma) {
        if CaptureModule::is_capture(vboma) && CaptureModule::catch_object_id(vboma) as u32 == me {
            return Some((id, "capture-link"));
        }
        if StatusModule::status_kind(vboma) == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED {
            return Some((id, "caught-status"));
        }
    }
    None
}

/// Whoever is standing inside the catch region, ignoring whether the engine noticed.
///
/// THE LAST RESORT, AND IT IS ONLY REACHED IF THE ENGINE DECLINES THE CATCH ENTIRELY.
///
/// A CATCH collision belongs to the fighter that created it, and there is no guarantee the engine
/// will link one created by a Pikachu running Incineroar's script -- the native handler that
/// completes an Alolan Whip checks the fighter kind. If it does link, `find_caught_victim` sees it
/// and this is never called. If it does not, the alternative is a move that can never connect, so
/// this reproduces the test geometrically from the ACMD's own numbers.
///
/// THE REGION IS THE ACMD'S, NOT AN INVENTION. game_specialsstart makes two catch boxes on `top`:
///
///   id 0   size 4.166667   at (0, 8.333334, 2.0833335) .. (0, 8.333334, 8.333334)
///   id 1   size 1.5625001  at (0, 8.333334, 2.0833335) .. (0, 8.333334, 10.937501)
///
/// On `top`, Z is forward and Y is up, and every figure is Incineroar's own divided by Pikachu's
/// 0.96 `scale` so it lands in the same world space. Their union reaches from 2.08 to 10.94
/// forward, with the wide box covering 8.33 +/- 4.17 vertically. So: in front, within
/// `CATCH_REACH_Z` forward of us, and with the victim's own body -- feet at pos_y, so the check
/// allows anything whose root is below the top of the box -- inside the vertical span.
///
/// It deliberately does NOT grab someone another fighter is already holding, or anyone who is not
/// in a state a grab can take (`is_capture` covers being held; a dead or respawning fighter is
/// not a live battle object and never reaches here).
const CATCH_REACH_Z: f32 = 10.937501;
const CATCH_TOP_Y: f32 = 8.333334 + 4.166667;

unsafe fn find_victim_in_range(
    boma: *mut BattleObjectModuleAccessor,
) -> Option<(u32, *mut BattleObjectModuleAccessor)> {
    let lr = PostureModule::lr(boma);
    let (mx, my) = (PostureModule::pos_x(boma), PostureModule::pos_y(boma));
    let mut best: Option<(f32, u32, *mut BattleObjectModuleAccessor)> = None;
    for (id, vboma) in other_fighters(boma) {
        if CaptureModule::is_capture(vboma) {
            continue;
        }
        // A fighter who is dead, respawning or still on the platform is a live battle object with
        // a position, and grabbing one would be a bug the engine's own catch would never make.
        let st = StatusModule::status_kind(vboma);
        if st == *FIGHTER_STATUS_KIND_DEAD
            || st == *FIGHTER_STATUS_KIND_REBIRTH
            || st == *FIGHTER_STATUS_KIND_ENTRY
        {
            continue;
        }
        let forward = (PostureModule::pos_x(vboma) - mx) * lr;
        let dy = PostureModule::pos_y(vboma) - my;
        if forward < 0.0 || forward > CATCH_REACH_Z || dy < -CATCH_TOP_Y || dy > CATCH_TOP_Y {
            continue;
        }
        if best.map(|(d, _, _)| forward < d).unwrap_or(true) {
            best = Some((forward, id, vboma));
        }
    }
    best.map(|(_, id, vboma)| (id, vboma))
}

unsafe fn entry_of(boma: *mut BattleObjectModuleAccessor) -> usize {
    let e = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if e >= 8 { 0 } else { e }
}

unsafe fn reset(boma: *mut BattleObjectModuleAccessor) {
    let e = entry_of(boma);
    VICTIM[e] = *BATTLE_OBJECT_ID_INVALID as u32;
    HELD_FOR[e] = 0;
    SINCE_THROW[e] = -1;
    PULLIN_SEEN[e] = false;
}

/// The cached victim's module accessor, or None if they are gone.
/// Validated through `crate::victim`, which additionally requires the accessor to agree about
/// its own battle object id -- see that module for why an id-shaped check is not enough.
unsafe fn victim_boma(entry: usize) -> Option<*mut BattleObjectModuleAccessor> {
    crate::victim::boma(VICTIM[entry])
}

/// Horizontal distance from Pikachu to the cached victim, or None if there is no live victim.
unsafe fn victim_gap(boma: *mut BattleObjectModuleAccessor) -> Option<f32> {
    let vboma = victim_boma(entry_of(boma))?;
    Some((PostureModule::pos_x(vboma) - PostureModule::pos_x(boma)).abs())
}

/// Moves the victim one frame along the whip-and-rebound arc.
///
/// `frames_since` counts from FLAG_THROW_ROPE. Out at `through_target_speed` for
/// `through_target_cancel_frame` frames, then back at `through_target_return_speed` -- all three
/// numbers Incineroar's own. Direction is away from Pikachu's facing on the way out and toward him
/// coming back, which is what puts them in the lariat band roughly nine frames before contact.
///
/// Position rather than velocity on purpose: a scripted throw should trace the same arc regardless
/// of how heavy the victim is or what their own kinetic state was doing.
unsafe fn drive_victim(boma: *mut BattleObjectModuleAccessor, frames_since: i32) {
    let entry = entry_of(boma);
    // Stops the moment they are no longer ours -- see `victim_still_ours`. An opponent who mashed
    // free mid-flight should fall, not keep tracing the rest of the arc.
    if !victim_still_ours(boma, entry) {
        return;
    }
    let Some(vboma) = victim_boma(entry) else { return };

    let lr = PostureModule::lr(boma);
    let outbound = frames_since <= THROUGH_CANCEL_FRAME;
    let step = if outbound { THROUGH_SPEED } else { -THROUGH_RETURN_SPEED };

    // Their own physics would fight this every frame, so clear it first. The victim is in a
    // SWING_GAOGAEN status, which is a scripted capture -- they are not meant to be self-propelled.
    KineticModule::clear_speed_all(vboma);
    PostureModule::add_pos(vboma, &Vector3f { x: step * lr, y: 0.0, z: 0.0 });
}

/// An attack or special press this frame. SmashWiki says "an attack button", and in practice both
/// A and B commit the follow-up, so both are accepted.
unsafe fn followup_pressed(boma: *mut BattleObjectModuleAccessor) -> bool {
    ControlModule::check_button_trigger(boma, *CONTROL_PAD_BUTTON_ATTACK)
        || ControlModule::check_button_trigger(boma, *CONTROL_PAD_BUTTON_SPECIAL)
}

// ---------------------------------------------------------------------------------------------
// Shared status scaffolding. Every one of the six statuses wants the same init_settings and
// set_fighter_status_data, differing only in whether it starts stationary, so they share these.
// ---------------------------------------------------------------------------------------------

unsafe fn common_pre(fighter: &mut L2CFighterCommon, kinetic: i32) {
    StatusModule::init_settings(
        fighter.module_accessor,
        smash::app::SituationKind(*SITUATION_KIND_NONE),
        kinetic,
        GROUND_CORRECT_KIND_KEEP.into(),
        smash::app::GroundCliffCheckKind(*GROUND_CLIFF_CHECK_KIND_NONE),
        true,
        FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLAG.into(),
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_INT,
        FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLOAT.into(),
        0
    );
    FighterStatusModuleImpl::set_fighter_status_data(
        fighter.module_accessor,
        false,
        *FIGHTER_TREADED_KIND_NO_REAC,
        false,
        false,
        false,
        (*FIGHTER_LOG_MASK_FLAG_ACTION_TRIGGER_ON | *FIGHTER_LOG_MASK_FLAG_ATTACK_KIND_SPECIAL_S) as u64,
        0,
        FIGHTER_POWER_UP_ATTACK_BIT_SPECIAL_S.into(),
        0
    );
}

/// AIR OR GROUND, per Incineroar. He has a complete airborne Alolan Whip -- six motions, six ACMDs
/// and its own params (special_air_s_catch_accel_y 0.01, _speed_y_max 0.05: he drifts slowly down
/// while holding) -- and this port played the GROUNDED animation and grounded hitboxes for an
/// airborne side-B until his motion list was read instead of assumed.
///
/// The two names differ only by the `air` infix on every one of the six, so one helper covers all
/// of them and each status just names its own suffix.
unsafe fn situated(fighter: &mut L2CFighterCommon, suffix: &str) -> String {
    if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
        format!("special_s{suffix}")
    } else {
        format!("special_air_s{suffix}")
    }
}

/// Ground/air housekeeping shared by the follow-ups, which are all performed standing still.
unsafe fn hold_still(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    if fighter.global_table[0x16] != *SITUATION_KIND_GROUND {
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_AIR_STOP);
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
    } else {
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_GROUND_STOP);
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
    }
}

/// Hand control back once a follow-up's animation is done.
unsafe fn finish(fighter: &mut L2CFighterCommon) -> L2CValue {
    if MotionModule::is_end(fighter.module_accessor) {
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_END.into(), false.into());
        return 1.into();
    }
    0.into()
}

// ---------------------------------------------------------------------------------------------
// SPECIAL_S -- the lunge.
// ---------------------------------------------------------------------------------------------

unsafe extern "C" fn pikachu_specials_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    // FIGHTER_KINETIC_TYPE_MOTION, not UNIQ. The lunge is root motion out of
    // d01specialsstart.nuanmb -- 34.93 units of it in the Trans track -- scaled by
    // special_s_start_speed_mul. The old port used UNIQ and then immediately changed to
    // GROUND_STOP in Main, which pinned him in place: "it also no longer travels anywhere".
    common_pre(fighter, *FIGHTER_KINETIC_TYPE_MOTION);
    0.into()
}

unsafe extern "C" fn pikachu_specials_init(fighter: &mut L2CFighterCommon) -> L2CValue {
    // special_s_start_speed_mul = 1.3, applied to the motion energy exactly as G&W's up-B applies
    // his own 1.1 in ../specialhi.
    fighter.agent.clear_lua_stack();
    fighter.agent.push_lua_stack(&mut L2CValue::new_int(*FIGHTER_KINETIC_ENERGY_ID_MOTION as u64));
    fighter.agent.push_lua_stack(&mut L2CValue::new_num(START_SPEED_MUL));
    sv_kinetic_energy::set_speed_mul(fighter.lua_state_agent);
    0.into()
}

unsafe extern "C" fn pikachu_specials_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    reset(boma);
    MotionModule::change_motion(
        boma, Hash40::new(&situated(fighter, "_start")), 0.0, 1.0, false, 0.0, false, false
    );
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_specials_main_loop as *const () as _))
}

unsafe extern "C" fn pikachu_specials_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    use crate::telemetry::probe;
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    let mframe = MotionModule::frame(boma);

    // THE CATCH WINDOW, AND THE ONE MEASUREMENT NOBODY HAD EVER TAKEN.
    //
    // Three rounds of comments here asserted that "is_catch never goes true, measured across a
    // whole session". Going back to the log that was read off: every WHIP line in it stops at
    // motion frame 17, because the build that wrote them triggered on `GrabModule::is_grab` and
    // `is_grab` goes true on 17. `macros::CATCH` does not run until script frame 18. **The catch
    // window had never been observed at all.** Everything asserted about what the engine does or
    // does not report during it was inference dressed as measurement.
    //
    // So this logs the window unconditionally and nothing here exits before the end of it. The
    // per-frame line now carries both sides:
    //
    //   ours     is_catch / capture_object_id / rebound / both grab collision sizes and nodes,
    //            which together say whether the ACMD ran and whether the engine thinks the
    //            collision holds anyone
    //   theirs   for each live opponent: status, CaptureModule::is_capture, catch_object_id and
    //            forward distance, which says whether one was in range and what the engine did
    //            to them
    //
    // That is the whole decision table in one line per frame, and it cannot be read two ways.
    let is_catch = CatchModule::is_catch(boma);
    let cap = CatchModule::capture_object_id(boma, false) as u32;
    // The window the ACMD opens (script frame 18) and clears (35). Logged a little wider so the
    // edges are visible rather than inferred.
    if (14.0..40.0).contains(&mframe) {
        let lr = PostureModule::lr(boma);
        let mx = PostureModule::pos_x(boma);
        let mut them = String::new();
        for (id, vboma) in other_fighters(boma) {
            them.push_str(&format!(
                " [{id:#x} st={:#x} cap={} by={:#x} fwd={:.1}]",
                StatusModule::status_kind(vboma),
                CaptureModule::is_capture(vboma),
                CaptureModule::catch_object_id(vboma) as u32,
                (PostureModule::pos_x(vboma) - mx) * lr,
            ));
        }
        probe::frame(format!(
            "[rat] WHIP p{entry} f={mframe:.1} is_catch={is_catch} cap={cap:#x} rebound={} \
             grab0={} grab1={} size0={:#x} size1={:#x} node0={:#x} node1={:#x} pullin_hit={} \
             them:{them}",
            GrabModule::is_rebound(boma),
            GrabModule::is_grab(boma, 0),
            GrabModule::is_grab(boma, 1),
            GrabModule::size(boma, 0),
            GrabModule::size(boma, 1),
            GrabModule::node(boma, 0),
            GrabModule::node(boma, 1),
            AttackModule::is_attack_occur(boma),
        ));
    }

    // THE CATCH ITSELF, IN TWO TIERS.
    //
    // Tier one is the engine: a capture link naming us, or somebody sitting in
    // SWING_GAOGAEN_CATCHED. If either is true the engine completed the catch and we only have
    // to notice.
    //
    // Tier two exists because it may never be true. `tools/disasm` establishes that Incineroar's
    // own scripts never detect this either -- the native handler does, and it runs because the
    // fighter is Gaogaen. There is no guarantee the engine will link a CATCH collision created by
    // a Pikachu, and if it does not, no amount of asking better questions will produce a grab.
    // So when the collision is open and an opponent is standing inside the region the ACMD
    // describes, the catch is taken here.
    //
    // THIS IS NOT A SHORTCUT AROUND THE ENGINE, BECAUSE THE PORT ALREADY OWNS THE VICTIM. The
    // rope trip is driven from SPECIAL_S_THROW by writing the victim's posture with Incineroar's
    // own `special_s_through_target_*` numbers -- see the header -- precisely because Pikachu's
    // vl.prc has no param_special_s for the engine's SWING statuses to read. Once that is true,
    // the capture link buys nothing the port uses. Who was caught, and when, is all it needs.
    //
    // Tier two is gated on the window the ACMD actually opens (script frame 18 to 35) so it
    // cannot fire during the startup or the recovery, and it reports itself distinctly in the log
    // so a session where the engine DID link is never confused with one where it did not.
    let mut caught = find_caught_victim(boma);
    if caught.is_none() && is_catch && cap != *BATTLE_OBJECT_ID_INVALID as u32 {
        caught = Some((cap, "catcher-link"));
    }
    // FROM FRAME 15, NOT 18, AND THE LOG SAYS WHY IN ONE LINE PER FRAME.
    //
    //   f=17.0 ... pullin_hit=true  them: [0x10000 st=0x2a cap=false fwd=5.0]
    //   f=18.0 ... pullin_hit=true  them: [0x10000 st=0x2a cap=false fwd=12.8]
    //   f=19.0 ...                  them: [... fwd=16.1]
    //
    // `fwd` is the opponent's distance in front of us. On frame 17 they are 5.0 away -- well
    // inside CATCH_REACH_Z of 10.94 -- and our pull-in hitbox connects. By frame 18, which is
    // where this window used to open, they are 12.8 away and outside it, and they keep going.
    // The ACMD's frame-15 and frame-16 ATTACKs are Incineroar's pull-in, angle 350 with FKB 30,
    // and on Pikachu they knock the opponent clear of the catch region before the catch boxes
    // exist. Every frame from 18 to 35 then correctly reports nobody in range, which is why the
    // whip "grabs sometimes" and mostly does not.
    //
    // So the window opens where the ACMD's first hitbox does. Frames 15 to 17 are the lunge
    // closing the distance and 18 to 35 are the catch boxes; treating the whole span as the
    // grab's active window is what the move is, and the geometry check still decides.
    //
    // THE PULL-IN CONNECTING IS ALSO A SIGNAL IN ITS OWN RIGHT. It means an opponent was in the
    // same region the catch boxes are about to occupy, which is exactly the question the catch
    // asks -- and it is true on the frame BEFORE they get launched out of it.
    // `is_attack_occur` is sticky once it fires, so only the first frame counts.
    let pullin = AttackModule::is_attack_occur(boma);
    let pullin_edge = pullin && !PULLIN_SEEN[entry];
    if (14.0..=40.0).contains(&mframe) {
        PULLIN_SEEN[entry] = pullin;
    }
    if caught.is_none() && ((15.0..=35.0).contains(&mframe) || pullin_edge) {
        if let Some((id, vboma)) = find_victim_in_range(boma) {
            StatusModule::change_status_request_from_script(
                vboma, *FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED, false
            );
            caught = Some((id, "in-range"));
        }
    }
    if let Some((id, how)) = caught {
        VICTIM[entry] = id;
        probe::event(format!(
            "[rat] WHIP p{entry} CAUGHT {id:#x} via {how} on f={mframe:.1} \
             (is_catch={is_catch} cap={cap:#x}); {}",
            probe::ctx(boma)
        ));
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_THROW.into(), false.into());
        return 1.into();
    }

    // A WHIFF ENDS AT THE MOVE'S OWN FAF, NOT AT THE END OF THE ANIMATION.
    //
    // This waited for `MotionModule::is_end`, and `special_s_start` is a 117-frame animation
    // because it covers the whole HOLD -- Incineroar keeps posing while he has someone. Playtest
    // measured exactly that: 117 frames of lunge, then 44 more of special_s_end, about two and
    // three quarter seconds for a whiffed side-B, and it read as "the enemy escaped animation
    // when that's not native behavior".
    //
    // Incineroar's own entry says what a whiff costs: `special_s_start` has a cancel_frame of 64.
    // Read at runtime from the shipped motion_list rather than hardcoded, so it tracks whatever
    // motion-patch put there.
    //
    // AND IT GOES THROUGH SPECIAL_S_END, WHICH IS A THIRD ANSWER TO A QUESTION ASKED TWICE.
    //
    //   round 2  waited for `is_end` on a 117-frame animation and THEN played special_s_end.
    //            161 frames. Playtest: "the much lengthier escape animation as well".
    //   round 3  exited at the FAF straight to WAIT. 64 frames. Playtest: "the end animation is
    //            mostly correct -- it doesn't seem to have enough end lag".
    //   now      exits at the FAF into SPECIAL_S_END, whose own entry has a cancel_frame of 20.
    //            About 84 frames to actionable with the recovery animation actually playing.
    //
    // Both bounds are measured and the middle is the only reading consistent with both reports:
    // the lunge should not run its full length on a miss, and cutting straight to WAIT drops the
    // recovery entirely. `special_s_end` is the move's own recovery motion -- round 2 was wrong
    // that playing it was the mistake; playing it AFTER 117 frames of lunge was.
    //
    // The tracer will settle it exactly: the same telemetry is on Incineroar now, so one whiffed
    // side-B as the real character gives the status sequence and the frame counts to diff against.
    let faf = FighterMotionModuleImpl::get_cancel_frame(boma, Hash40::new(&situated(fighter, "_start")), true);
    if (faf >= 1.0 && mframe >= faf) || MotionModule::is_end(boma) {
        probe::event(format!(
            "[rat] WHIP p{entry} whiffed at f={mframe:.1} (faf {faf:.0}) -> SPECIAL_S_END; {}",
            probe::ctx(boma)
        ));
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_END.into(), false.into());
        return 1.into();
    }
    0.into()
}

unsafe extern "C" fn pikachu_specials_end(fighter: &mut L2CFighterCommon) -> L2CValue {
    let _ = fighter;
    0.into()
}

// ---------------------------------------------------------------------------------------------
// SPECIAL_S_THROW -- the Irish whip, and the window in which the follow-up is chosen.
// ---------------------------------------------------------------------------------------------

unsafe extern "C" fn pikachu_specialsthrow_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    common_pre(fighter, *FIGHTER_KINETIC_TYPE_UNIQ);
    0.into()
}

unsafe extern "C" fn pikachu_specialsthrow_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    HELD_FOR[entry_of(boma)] = 0;
    SINCE_THROW[entry_of(boma)] = -1;
    MotionModule::change_motion(
        boma, Hash40::new(&situated(fighter, "_throw")), 0.0, 1.0, false, 0.0, false, false
    );
    hold_still(fighter);
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_specialsthrow_main_loop as *const () as _))
}

/// Keep the victim in Incineroar's grip for the frames before the whip.
///
/// The engine's capture does this when it owns the catch: SWING_GAOGAEN_CATCHED follows the
/// master. When the catch was taken here instead -- see `find_victim_in_range` -- there is no
/// master to follow and nothing would stop the victim walking out of the hold during the two
/// dozen frames before FLAG_THROW_ROPE. So this does the same job the capture would.
///
/// It runs unconditionally rather than only for a forced catch. If the engine IS holding them,
/// it is writing the same position this does a fraction of a unit away, and the cost of agreeing
/// with it is nothing; the cost of getting the condition wrong is a victim who slides.
///
/// FORWARD BY THE CATCH BOX'S OWN NEAR EDGE. The wide catch collision sits at z 2.08 with radius
/// 4.17, so its centre of mass is about 4 units in front of `top` -- which is where an opponent
/// it just caught actually is. Holding them there rather than at some invented distance means
/// the whip starts from the position the grab ended at.
const HOLD_FORWARD: f32 = 4.166667;

/// Is the cached victim still ours to move?
///
/// A STUCK GRAB IS WORSE THAN A MISSED ONE, and without this the port could produce one.
/// `victim_boma` proves the handle is a live fighter; it says nothing about whether that fighter
/// is still caught. Incineroar's victim can leave at any moment -- `special_s_catch_clatter_frame`
/// is 85 frames of mashing, and `status_SwingGaogaenCatched_Main` in lua2cpp_common.nro reads
/// `ControlModule::get_clatter_time` and transitions them out itself when it runs down.
///
/// Whoever escaped that way was still being written by `pin_victim` and `drive_victim` every
/// frame: position overwritten, `clear_speed_all` called, for the rest of the move. Held in place
/// by a grab they had already broken, with no input that could free them.
///
/// So both writers ask first, and the answer is the same pair of questions the catch used: are
/// they in the caught status, or does the engine's capture link still name us.
unsafe fn victim_still_ours(boma: *mut BattleObjectModuleAccessor, entry: usize) -> bool {
    let Some(vboma) = victim_boma(entry) else { return false };
    if StatusModule::status_kind(vboma) == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED {
        return true;
    }
    CaptureModule::is_capture(vboma)
        && CaptureModule::catch_object_id(vboma) as u32 == (*boma).battle_object_id
}

unsafe fn pin_victim(boma: *mut BattleObjectModuleAccessor, entry: usize) {
    if !victim_still_ours(boma, entry) {
        return;
    }
    let Some(vboma) = victim_boma(entry) else {
        return;
    };
    let pos = Vector3f {
        x: PostureModule::pos_x(boma) + PostureModule::lr(boma) * HOLD_FORWARD,
        y: PostureModule::pos_y(boma),
        z: 0.0,
    };
    PostureModule::set_pos(vboma, &pos);
    // Their own momentum must not fight the placement, or they drift out of the grip.
    KineticModule::clear_speed_all(vboma);
}

unsafe extern "C" fn pikachu_specialsthrow_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);

    // THE WHIP. game_specialsthrow raises FLAG_THROW_ROPE on its frame 37; deforeign keeps that
    // id now (see tools/reach/src/bin/deforeign.rs KEEP_STATUS_IDS) so it can be read here rather
    // than approximated. The frame fallback covers the case where the flag never arrives.
    if SINCE_THROW[entry] < 0 {
        HELD_FOR[entry] += 1;
        pin_victim(boma, entry);
        let thrown = WorkModule::is_flag(boma, *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_THROW_ROPE)
            || HELD_FOR[entry] >= THROW_ROPE_FALLBACK_FRAME;
        if thrown {
            SINCE_THROW[entry] = 0;
            // THE VICTIM STAYS IN SWING_GAOGAEN_CATCHED. HANDING THEM TO THE ENGINE'S ROPE
            // SEQUENCE IS A CONFIRMED CRASH.
            //
            // This used to request SWING_GAOGAEN_THROWN, on the reasoning that the outbound leg,
            // the bounce and the return are all engine-common statuses and therefore the game's
            // code rather than ours. The Atmosphere report says what that actually did:
            //
            //   Data Abort, fault address 0
            //     L2CFighterCommon::status_SwingGaogaenThrown + 0xf8
            //     -> app::lua_bind::LinkModule__get_parent_pos_impl
            //
            // `tools/disasm` on lua2cpp_common.nro, pulled from the running game, shows the
            // instruction: `get_parent_pos` returns the position of the LINKED PARENT -- for
            // Incineroar, always his ROPE article -- and the next three instructions read x, y
            // and z straight off the returned pointer with no null check. Pikachu has no rope,
            // so there is no link, so the pointer is null.
            //
            // It was never salvageable by giving the victim a different status either: all three
            // rope statuses are the rope. So the trajectory is driven from here, which the
            // header already explains was necessary anyway -- Pikachu's vl.prc has no
            // param_special_s for those statuses to read their speeds out of. See src/victim.rs
            // for the refusal list; CATCHED is safe because it guards every parent access behind
            // LinkModule::is_link.
            if let Some(vboma) = crate::victim::boma(VICTIM[entry]) {
                crate::telemetry::sink::crumb(format!(
                    "[rat] WHIP p{entry} rope moment: victim {:#x} stays in status {:#x}, \
                     trajectory driven by drive_victim",
                    VICTIM[entry],
                    StatusModule::status_kind(vboma),
                ));
            }
        } else if HELD_FOR[entry] >= CATCH_CLATTER_FRAME {
            // special_s_catch_clatter_frame: the victim mashed out before the whip.
            fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_END.into(), false.into());
            return 1.into();
        }
        return 0.into();
    }

    SINCE_THROW[entry] += 1;
    // THEY MAY HAVE MASHED OUT MID-FLIGHT, and that is a state the move has to answer for.
    // `status_SwingGaogaenCatched_Main` in lua2cpp_common.nro reads
    // `ControlModule::get_clatter_time` and releases the victim itself when it runs down --
    // `special_s_catch_clatter_frame` is 85 frames of it. "Keep driving a fighter who is no
    // longer caught" is not an answer: it holds them in place through a grab they already broke.
    // Incineroar simply finishes the move, and SPECIAL_S_END is that.
    if !victim_still_ours(boma, entry) {
        crate::telemetry::probe::event(format!(
            "[rat] WHIP p{entry} victim escaped {} frame(s) after the whip -- ending; {}",
            SINCE_THROW[entry],
            crate::telemetry::probe::ctx(boma)
        ));
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_END.into(), false.into());
        return 1.into();
    }
    // Carry the victim out to the ropes and back. Without this the engine's own SWING_GAOGAEN
    // statuses would try to, and they read numbers Pikachu's vl.prc does not have.
    drive_victim(boma, SINCE_THROW[entry]);

    // THE FOLLOW-UP DECISION, off the three param regions.
    let gap = victim_gap(boma);
    if followup_pressed(boma) {
        let in_lariat_band = SINCE_THROW[entry] >= RETURN_GUARD_FRAMES
            && gap.map_or(false, |g| g <= LARIAT_X && g >= LARIAT_END_X);
        let next = if in_lariat_band {
            RAT_STATUS_KIND_SPECIAL_S_LARIAT
        } else {
            RAT_STATUS_KIND_SPECIAL_S_SHOULDER
        };
        fighter.change_status(next.into(), false.into());
        return 1.into();
    }

    // No press, and the rebound has arrived. special_s_through_target_cancel_frame gates this so
    // the check cannot fire while the victim is still being held at zero distance.
    if SINCE_THROW[entry] > THROUGH_CANCEL_FRAME {
        let arrived = gap.map_or(true, |g| g <= FAILURE_X);
        if arrived || MotionModule::is_end(boma) {
            fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_FAILURE.into(), false.into());
            return 1.into();
        }
    }

    if MotionModule::is_end(boma) {
        fighter.change_status(RAT_STATUS_KIND_SPECIAL_S_FAILURE.into(), false.into());
        return 1.into();
    }
    0.into()
}

// ---------------------------------------------------------------------------------------------
// The three follow-ups. Each is one motion plus its ACMD, which carries the whole hitbox and the
// ATTACK_ABS that releases the victim -- so the status code has nothing to do but play it.
// ---------------------------------------------------------------------------------------------

unsafe extern "C" fn pikachu_specialsshoulder_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    common_pre(fighter, *FIGHTER_KINETIC_TYPE_UNIQ);
    0.into()
}

unsafe extern "C" fn pikachu_specialsshoulder_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    MotionModule::change_motion(
        fighter.module_accessor, Hash40::new(&situated(fighter, "_shoulder")), 0.0, 1.0, false, 0.0, false, false
    );
    hold_still(fighter);
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_followup_loop as *const () as _))
}

unsafe extern "C" fn pikachu_specialslariat_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    MotionModule::change_motion(
        fighter.module_accessor, Hash40::new(&situated(fighter, "_lariat")), 0.0, 1.0, false, 0.0, false, false
    );
    hold_still(fighter);
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_followup_loop as *const () as _))
}

unsafe extern "C" fn pikachu_specialsfailure_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    // special_s_fail_damage = 4. Both sides take it; the victim's share is the ATTACK in
    // game_specialsfailure, this is his.
    DamageModule::add_damage(boma, FAIL_DAMAGE, 0);
    MotionModule::change_motion(
        boma, Hash40::new(&situated(fighter, "_failure")), 0.0, 1.0, false, 0.0, false, false
    );
    hold_still(fighter);
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_followup_loop as *const () as _))
}

unsafe extern "C" fn pikachu_followup_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    finish(fighter)
}

// ---------------------------------------------------------------------------------------------
// SPECIAL_S_END -- the recovery every path funnels into.
// ---------------------------------------------------------------------------------------------

unsafe extern "C" fn pikachu_specialsend_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    common_pre(fighter, *FIGHTER_KINETIC_TYPE_UNIQ);
    0.into()
}

unsafe extern "C" fn pikachu_specialsend_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    reset(boma);
    MotionModule::change_motion(
        boma, Hash40::new(&situated(fighter, "_end")), 0.0, 1.0, false, 0.0, false, false
    );
    hold_still(fighter);
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_specialsend_main_loop as *const () as _))
}

unsafe extern "C" fn pikachu_specialsend_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    if MotionModule::is_end(boma) {
        if StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND {
            fighter.change_status(FIGHTER_STATUS_KIND_WAIT.into(), false.into());
        } else {
            fighter.change_status(FIGHTER_STATUS_KIND_FALL.into(), false.into());
        }
        return 1.into();
    }
    0.into()
}

pub fn install() {
    Agent::new("pikachu")
        .status(Pre,  *FIGHTER_STATUS_KIND_SPECIAL_S, pikachu_specials_pre)
        .status(Init, *FIGHTER_STATUS_KIND_SPECIAL_S, pikachu_specials_init)
        .status(Main, *FIGHTER_STATUS_KIND_SPECIAL_S, pikachu_specials_main)
        .status(End,  *FIGHTER_STATUS_KIND_SPECIAL_S, pikachu_specials_end)
        .status(Pre,  RAT_STATUS_KIND_SPECIAL_S_THROW, pikachu_specialsthrow_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_THROW, pikachu_specialsthrow_main)
        .status(Pre,  RAT_STATUS_KIND_SPECIAL_S_SHOULDER, pikachu_specialsshoulder_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_SHOULDER, pikachu_specialsshoulder_main)
        .status(Pre,  RAT_STATUS_KIND_SPECIAL_S_LARIAT, pikachu_specialsshoulder_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_LARIAT, pikachu_specialslariat_main)
        .status(Pre,  RAT_STATUS_KIND_SPECIAL_S_FAILURE, pikachu_specialsshoulder_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_FAILURE, pikachu_specialsfailure_main)
        .status(Pre,  RAT_STATUS_KIND_SPECIAL_S_END, pikachu_specialsend_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_S_END, pikachu_specialsend_main)
        .install();
}
