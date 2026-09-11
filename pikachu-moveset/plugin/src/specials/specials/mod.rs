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
pub const CATCH_CLATTER_FRAME: i32 = 85;
const FAIL_DAMAGE: f32 = 4.0;
pub const THROUGH_CANCEL_FRAME: i32 = 30;
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
/// `special_s_failure_offset_x` -- the rebound has arrived.
const FAILURE_X: f32 = 6.0;
/// `special_s_lariat_motion_rate_min` and `special_s_lariat_motion_rate_max`. See `lariat_rate`.
const LARIAT_RATE_MIN: f32 = 1.0;
const LARIAT_RATE_MAX: f32 = 2.0;
/// How far behind Pikachu the ropes stand.
///
/// NOT A PARAM, AND THAT IS WHY IT HAD TO BE DERIVED. Nothing in `param_special_s` gives the
/// rope distance -- the ropes are an ARTICLE, and their placement lives in the article. SmashWiki
/// puts them at roughly where the dash started, and `d01specialsstart` carries 34.93 units of
/// root motion, so that is the distance the lunge covers and the distance back to them.
///
/// Two independent readings of the params agree with it, which is what makes it usable rather
/// than a guess: 34.93 / `through_target_speed` 11.2 is about three frames outbound, and the
/// LARIAT band at -24 divided by `through_target_return_speed` 2.8 is 8.6 frames -- against
/// game_specialslariat putting its hitbox out on frame 9. The geometry and the timing were
/// derived separately and land on the same move.
const ROPE_DISTANCE: f32 = 34.93;

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
/// Whether the whipped victim has reached the ropes and turned around.
static mut RETURNING: [bool; 8] = [false; 8];
/// Whether the engine has ever raised one of its own follow-up area flags for us.
static mut AREA_FLAG_SEEN: [bool; 8] = [false; 8];

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
    crate::victim::release(e, "side-B restarted");
    VICTIM[e] = *BATTLE_OBJECT_ID_INVALID as u32;
    HELD_FOR[e] = 0;
    SINCE_THROW[e] = -1;
    PULLIN_SEEN[e] = false;
    RETURNING[e] = false;
    AREA_FLAG_SEEN[e] = false;
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

    // THE FLIGHT IS HAND-DRIVEN, SO THE ENGINE MUST LET GO FIRST.
    //
    // The hold phase hands them to the engine -- see `take_captive` -- and the model constraint
    // that keeps them in the grip would fight every `add_pos` below. Idempotent: after the first
    // frame the capture is gone and this is two virtual calls that answer no.
    crate::victim::cut_capture(entry, "the whip's rope flight is driven by hand");

    // BEHIND HIM, NOT IN FRONT, AND THE PARAMS SAY SO IN EVERY SIGN.
    //
    //   special_s_lariat_offset_x1/x2      -24
    //   special_s_lariat_end_offset_x1/x2  -10
    //   special_s_failure_offset_x1/x2      -6
    //
    // Every detection band is NEGATIVE -- behind Incineroar. The ropes rise up behind him, the
    // victim is whipped back past him into them, and comes back toward him from behind. This sent
    // them forward at +lr, which is why playtest saw Mario "flying the wrong direction".
    //
    // AND THE TURN IS A DISTANCE, NOT A TIMER. The old version flew outbound for
    // `through_target_cancel_frame` -- 30 frames at `through_target_speed` 11.2, which is 336
    // units. The victim was off the stage before the return leg began, so they "never bounce back
    // for the hit". The ropes are what turns them around, and SmashWiki puts them at roughly
    // where the dash started; `d01specialsstart` carries 34.93 units of root motion, so that is
    // the distance. 34.93 / 11.2 is about three frames out, then 2.8 per frame back.
    //
    // That also reconciles the lariat window, which had been derived from the numbers separately:
    // 24 units at return_speed 2.8 is 8.6 frames, and game_specialslariat's hitbox comes out on
    // frame 9. The band is exactly "a lariat started now connects as they arrive". Two independent
    // readings of the same params agreeing is the strongest confirmation available here.
    //
    // cancel_frame stays as the ceiling it is: if something stops the victim reaching the ropes,
    // the outbound leg still ends.
    let lr = PostureModule::lr(boma);
    let gap = (PostureModule::pos_x(vboma) - PostureModule::pos_x(boma)).abs();
    if !RETURNING[entry] && (gap >= ROPE_DISTANCE || frames_since > THROUGH_CANCEL_FRAME) {
        RETURNING[entry] = true;
        crate::telemetry::probe::event(format!(
            "[rat] WHIP p{entry} hit the ropes at {gap:.1} units after {frames_since}f -- \
             returning at {THROUGH_RETURN_SPEED}/frame; {}",
            crate::telemetry::probe::ctx(boma)
        ));
    }
    // Outbound is away from his facing; the return is toward it.
    let step = if RETURNING[entry] { THROUGH_RETURN_SPEED } else { -THROUGH_SPEED };

    // Their own physics would fight this every frame, so clear it first. This is a scripted
    // throw -- they are not meant to be self-propelled while it runs.
    KineticModule::clear_speed_all(vboma);
    PostureModule::add_pos(vboma, &Vector3f { x: step * lr, y: 0.0, z: 0.0 });
}

/// Hands the victim to the ENGINE, which is what a CATCH collision would have done if Pikachu's
/// native catch handler recognised this move.
///
/// WHY THE CATCH NEVER REGISTERED, and why that mattered more than it looked. The per-frame trace
/// said so all along and it was read as noise: `cap=0x50000000` is BATTLE_OBJECT_ID_INVALID, and
/// the victim's side said `cap=false`. The ACMD's CATCH collision connects -- `GrabModule::is_grab`
/// goes true -- but the capture BOOKKEEPING is done by fighter-kind-specific native code that
/// never runs for a Pikachu. So the victim was put into SWING_GAOGAEN_CATCHED with no capture
/// behind it, and that status spends its whole first frame asking about a capture:
///
///   LinkModule::is_link(LINK_NO_CAPTURE)          -> false, so no model constraint
///   LinkModule::get_parent_lr(LINK_NO_CAPTURE)    -> skipped, so they never face the right way
///   LinkModule::get_parent_id(LINK_NO_CAPTURE)    -> nothing, so the accessor is NULL and the
///                                                    twelve param_special_s reads never happen
///   ControlModule::start_clatter(frame, ...)      -> started from a frame count of 0
///
/// and `status_SwingGaogaenCatched_Main` is one test: when `get_clatter_time` reaches 0 it changes
/// them to FIGHTER_STATUS_KIND_CAPTURE_CUT. A clatter of zero frames expires on the frame it
/// starts. Playtest: "the opponent can still attack while the grab is happening."
///
/// THE CALL IS THE GAME'S, argument for argument. `status_MiifighterCounterThrown` is the only
/// place in lua2cpp_common that captures from a script, and it does exactly this:
///
///   LinkModule::unlink(boma, lua_const[0x4e4])            ; LINK_NO_CAPTURE
///   CaptureModule::capture(boma, parent_id, lua_const[0x250], true, 0)
///
/// with lua_const[0x250] resolving through vendor/ssbu-status/lua_dict.txt to STATUS_KIND_NONE.
/// The module is reached at `boma + 0x138` and the call is vtable slot 0x78 -- `capture(p1, p2,
/// bool p3, p4)` in the public decomp's CaptureModule.h -- on the CAPTURED object's own module, so
/// the accessor is the VICTIM's and the id is OURS. `STATUS_KIND_NONE` means "do not change their
/// status", which is why the status request stays here rather than being folded into the argument.
///
/// WHAT THIS BUYS, all of it native and none of it ours to get wrong: the model constraint that
/// holds them in the grip, their facing, the twelve params read off us through src/params, and an
/// 85-frame clatter they have to mash out of.
///
/// `crate::victim::release` performs the other half -- `CaptureModule::capture_cut` -- from the
/// one place every path goes through.
unsafe fn take_captive(
    boma: *mut BattleObjectModuleAccessor,
    vboma: *mut BattleObjectModuleAccessor,
    entry: usize,
) {
    let me = (*boma).battle_object_id;
    CaptureModule::capture(vboma, me, *STATUS_KIND_NONE, true, 0);
    let ok = CaptureModule::is_capture(vboma);
    let master = CaptureModule::catch_object_id(vboma) as u32;
    crate::victim::request_status(
        vboma,
        *FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED,
        "whip catch",
    );
    crate::telemetry::sink::diag(format!(
        "[rat] WHIP p{entry} capture requested on {:#x}: is_capture={ok} master={master:#x} \
         (ours is {me:#x}) -- if master is ours the victim's own status will read our \
         param_special_s and pin them for special_s_catch_clatter_frame",
        (*vboma).battle_object_id
    ));
}

/// An attack or special press this frame. SmashWiki says "an attack button", and in practice both
/// A and B commit the follow-up, so both are accepted.
unsafe fn followup_pressed(boma: *mut BattleObjectModuleAccessor) -> bool {
    // HELD, NOT TRIGGERED, because that is what the native loop reads.
    // `SpecialSThrow_main_loop` calls `ControlModule::check_button_on` in all four places it
    // tests for the follow-up -- four separate call sites, every one `_on`. A trigger is a
    // one-frame edge, so holding the button through the bounce did nothing unless the press
    // landed on exactly the right frame; `_on` is what makes "press early -> back body drop,
    // hold through the collision -> lariat" work at all.
    ControlModule::check_button_on(boma, *CONTROL_PAD_BUTTON_ATTACK)
        || ControlModule::check_button_on(boma, *CONTROL_PAD_BUTTON_SPECIAL)
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
            take_captive(boma, vboma, entry);
            caught = Some((id, "in-range"));
        }
    }
    if let Some((id, how)) = caught {
        VICTIM[entry] = id;
        // OURS NOW, on our own books -- see crate::victim. The engine keeps no record of a catch
        // it did not make, so nothing else in this move may ask it whether we are still holding
        // them.
        crate::victim::hold(entry, id);
        crate::victim::untrace(entry);
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

/// IN THE ORDER `SpecialSThrow_main` DOES IT. That function is eighteen lines of decompiled C and
/// this is now a transcription of it rather than a paraphrase:
///
///     pFVar4 = globalTable[5].as_pointer()          the FighterModuleAccessor
///     FighterSpecializer_Gaogaen::generate_rope(pFVar4)
///     if situation == GROUND { correct(GROUND_CLIFF_STOP); change_motion(special_s_throw) }
///     else                   { correct(AIR);               change_motion(special_air_s_throw) }
///     FUN_710001d4b0(this, true)                    the ground/air kinetic switch
///     sub_shift_status_main(SpecialSThrow_main_loop)
///
/// THE ROPE COMES FIRST AND IT IS THE WHOLE MOVE. Reading `SpecialSThrow_main_loop` settles what
/// six rounds of hand-writing could not: the rope throw is ARTICLE-DRIVEN. The loop raises
/// FLAG_THROW_ROPE_END and calls `LinkModule::send_event_nodes` twice -- the rope and the victim
/// are LINKED NODES, and the rope's own status is what moves them. The follow-up windows are not
/// distances anybody measures either; the engine raises FLAG_LARIAT_AREA, FLAG_LARIAT_END_AREA,
/// FLAG_LARIAT_AREA_FIRST and FLAG_FAILURE_AREA itself from those `-24 / -10 / -6` offsets, and
/// the loop just reads them. It also checks `ArticleModule::is_exist(rope)` and bails to
/// SPECIAL_S_END when the rope is gone.
///
/// So every part of this move that is currently reported as wrong -- "the timing of the
/// bounce-back and travel speed is off", "the opponent can still attack while the grab is
/// happening" -- is a consequence of the rope not existing, and all of it is downstream of one
/// call. `generate_rope` is exported from `main`, imported by Incineroar's OWN status NRO
/// (`disasm --sym rope` finds the PLT stub), and takes nothing but a module accessor.
///
/// Whether it works for a Pikachu is a measurement, and `try_native_rope` takes it.
unsafe extern "C" fn pikachu_specialsthrow_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    HELD_FOR[entry] = 0;
    SINCE_THROW[entry] = -1;
    try_native_rope(boma, entry);
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

/// Incineroar's ROPE ARTICLE ID. 0, from `FIGHTER_GAOGAEN_GENERATE_ARTICLE_ROPE`.
///
/// Written numerically for the same reason `crate::victim::FORBIDDEN_VICTIM_STATUSES` is: the
/// hand-written code may not name another fighter's constants (tools/check-status.sh), and the
/// point here is the number the ENGINE uses rather than a symbol we are entitled to.
const GAOGAEN_ARTICLE_ROPE: i32 = 0;

/// Whether calling the native rope generator produced a rope, per entry. Measured, not assumed.
static mut ROPE_EXISTS: [bool; 8] = [false; 8];

/// ASK THE GAME TO MAKE THE ROPES, USING ITS OWN FUNCTION.
///
/// THIS IS THE FIRST PIECE OF THE DRAG-AND-DROP THAT WAS SUPPOSED TO BE IMPOSSIBLE.
///
/// The whole reason this move is hand-written is that the three rope statuses --
/// SWING_GAOGAEN_THROWN, _ATTACH_ROPE, _RETURN -- dereference the victim's LINKED PARENT, and the
/// parent is Incineroar's rope article. No rope, null pointer, confirmed crash (ADR 0004). So the
/// trajectory is driven by hand with his numbers, and the reported cost of that is exactly what
/// you would expect from a reimplementation: "the timing of the bounce-back and travel speed is
/// off", and "the opponent can still attack while the grab is happening".
///
/// But the rope is not something only Incineroar's status code can make. `main` exports
///
///     app::FighterSpecializer_Gaogaen::generate_rope(FighterModuleAccessor&)
///
/// and skyline-smash already binds it. A `FighterModuleAccessor` is a
/// `BattleObjectModuleAccessor` as its first and only member, so Pikachu's accessor IS one.
///
/// WHETHER IT WORKS IS A MEASUREMENT, NOT A HOPE. `generate_rope` presumably calls
/// `ArticleModule::generate_article` for article 0, and Pikachu's article list is
/// dengekidama/kaminari/cloud/vortex -- his article 0 is not a rope. So this may no-op, may make
/// the wrong article, or may work. `ArticleModule::is_exist` immediately afterwards is what says
/// which, and nothing downstream acts on the rope unless that check passes. The hand-driven
/// trajectory stays exactly as it is until a log line says the rope is real.
///
/// If it IS real, the next step is to stop driving the victim at all and hand them to
/// SWING_GAOGAEN_THROWN -- the engine's own outbound leg, bounce and return, with the timings and
/// speeds that are currently "off" because they are mine rather than the game's.
unsafe fn try_native_rope(boma: *mut BattleObjectModuleAccessor, entry: usize) {
    let before = ArticleModule::is_exist(boma, GAOGAEN_ARTICLE_ROPE);
    crate::telemetry::sink::crumb(format!(
        "[rat] ROPE p{entry} about to call FighterSpecializer_Gaogaen::generate_rope \
         (article {GAOGAEN_ARTICLE_ROPE} exists before: {before})"
    ));
    smash::app::FighterSpecializer_Gaogaen::generate_rope(
        boma as *mut smash::app::FighterModuleAccessor,
    );
    let after = ArticleModule::is_exist(boma, GAOGAEN_ARTICLE_ROPE);
    ROPE_EXISTS[entry] = after;
    crate::telemetry::sink::crumb(format!(
        "[rat] ROPE p{entry} generate_rope returned; article {GAOGAEN_ARTICLE_ROPE} exists now: \
         {after}. If true, the engine's own SWING_GAOGAEN_THROWN becomes usable and the \
         hand-driven trajectory can go."
    ));
}

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
    // ASKED OF OURSELVES, NOT OF THE ENGINE, AND THE LOG IS WHY.
    //
    //   CAUGHT 0x10000 via in-range on f=16.0 (is_catch=false cap=0x50000000)
    //   rope moment: victim 0x10000 stays in status 0x44
    //   victim escaped 1 frame(s) after the whip -- ending
    //
    // This used to require SWING_GAOGAEN_CATCHED or a capture link naming us. The victim is in
    // 0x44 -- whatever the pull-in hitbox left them in -- and `cap=false by=0x50000000` on every
    // frame of every attempt. The engine did not make this grab and keeps no record of it, so
    // both tests were false one frame after the catch and this guard cancelled the throw itself.
    // Playtest: "it cancels and a failure animation plays".
    //
    // `crate::victim::held` is the fact instead, with a ceiling so it cannot become a stuck grab.
    // The engine's link is still accepted when it exists, because for an ordinary grab it does.
    let _ = boma;
    crate::victim::held(entry).is_some()
}

unsafe fn pin_victim(boma: *mut BattleObjectModuleAccessor, entry: usize) {
    if !victim_still_ours(boma, entry) {
        return;
    }
    let Some(vboma) = victim_boma(entry) else {
        return;
    };
    // IF THE ENGINE IS HOLDING THEM, DO NOTHING -- it is already doing this better than we can.
    //
    // `take_captive` establishes a real capture, and the victim's own `SwingGaogaenCatched` then
    // runs `LinkModule::set_model_constraint_pos_ort` with CONSTRAINT_FLAG_POSITION and
    // CONSTRAINT_FLAG_ORIENTATION every frame: they are attached to the grip by the same code
    // that attaches anyone Incineroar grabs. Writing their posture on top of that is two writers
    // fighting over one transform, which is the mistake `shoulder::hold_victim` already paid for.
    if crate::victim::engine_holds(entry, vboma) {
        return;
    }
    // The catch ends on THEIR side: `status_SwingGaogaenCatched_Main` escapes the moment
    // `ControlModule::get_clatter_time` hits zero, and that clock is started from OUR
    // `special_s_catch_clatter_frame` -- 85 for Incineroar, 0 for a Pikachu without
    // src/params. One line per change is what tells the two apart.
    crate::victim::trace(entry, vboma, "whip");
    let pos = Vector3f {
        x: PostureModule::pos_x(boma) + PostureModule::lr(boma) * HOLD_FORWARD,
        y: PostureModule::pos_y(boma),
        z: 0.0,
    };
    PostureModule::set_pos(vboma, &pos);
    // Their own momentum must not fight the placement, or they drift out of the grip.
    KineticModule::clear_speed_all(vboma);
}

/// Swaps between the grounded and airborne variant of a motion WHEN THE SITUATION CHANGES, without
/// restarting the animation.
///
/// `situated` picks a variant once, when the status is entered, so a whip that starts on the ground
/// and gets knocked into the air keeps playing the grounded animation for the rest of the move.
/// The source does not: `SpecialSThrow_main_loop` checks every frame and swaps, and it does it with
/// `change_motion_inherit_frame` rather than `change_motion` so the new animation picks up at the
/// frame the old one had reached instead of popping back to zero.
///
///   if globalTable[0x17] != GROUND && globalTable[0x16] == GROUND
///       GroundModule::correct(GROUND_CORRECT_KIND_GROUND_CLIFF_STOP)
///       change_motion_inherit_frame(hash40("special_s_throw"), -1.0, 1.0, 0.0, false, false)
///   elif globalTable[0x17] == GROUND && globalTable[0x16] != GROUND
///       GroundModule::correct(GROUND_CORRECT_KIND_AIR)
///       change_motion_inherit_frame(hash40("special_air_s_throw"), -1.0, 1.0, 0.0, false, false)
///
/// THE ENGINE ALREADY KEEPS THE PREVIOUS SITUATION, which is what makes this three lines instead of
/// a state machine: `global_table[0x16]` is the situation now and `global_table[0x17]` is what it
/// was last frame, so "did it just change" needs no bookkeeping of ours. `-1.0` for the frame is
/// what "inherit" means to that call.
///
/// `tools/check-nativecalls.sh` lists `MotionModule::change_motion_inherit_frame` against ten of
/// the twenty-one ported statuses; this is the one place the difference is visible in play, because
/// it is the only one of the whip's statuses long enough to be interrupted mid-animation.
unsafe fn follow_situation(fighter: &mut L2CFighterCommon, suffix: &str) {
    let boma = fighter.module_accessor;
    let ground = *SITUATION_KIND_GROUND;
    let now_ground = fighter.global_table[0x16] == ground;
    let was_ground = fighter.global_table[0x17] == ground;
    if now_ground == was_ground {
        return;
    }
    let (correct, motion) = if now_ground {
        (*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP, format!("special_s{suffix}"))
    } else {
        (*GROUND_CORRECT_KIND_AIR, format!("special_air_s{suffix}"))
    };
    GroundModule::correct(boma, smash::app::GroundCorrectKind(correct));
    MotionModule::change_motion_inherit_frame(
        boma, Hash40::new(&motion), -1.0, 1.0, 0.0, false, false,
    );
    crate::telemetry::probe::event(format!(
        "[rat] WHIP p{} situation changed mid-move -> {motion} at the frame it had reached; {}",
        entry_of(boma),
        crate::telemetry::probe::ctx(boma)
    ));
}

unsafe extern "C" fn pikachu_specialsthrow_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    follow_situation(fighter, "_throw");

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

    // THE ONE MEASUREMENT THAT DECIDES WHETHER THIS MOVE CAN BE A DRAG-AND-DROP.
    //
    // `SpecialSThrow_main_loop` does not measure distances. It reads four WORK FLAGS that the
    // engine raises from `special_s_lariat_offset_x`, `_lariat_end_offset_x` and
    // `_failure_offset_x` -- the -24 / -10 / -6 this file currently reimplements by hand, badly
    // enough that playtest calls the timing "off".
    //
    // If any of these ever reads true for a Pikachu, the native area detection is running for us
    // and every hand-derived number here can go: the bands, ROPE_DISTANCE, RETURNING, the whole
    // of `drive_victim`. If they stay false, the flags need the rope article and the answer is
    // whatever `try_native_rope` reported.
    //
    // Logged once per throw rather than per frame, and only when something is true, so a working
    // day looks like one line and a broken one looks like none.
    let area_flags = [
        ("LARIAT_AREA", *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_LARIAT_AREA),
        ("LARIAT_END_AREA", *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_LARIAT_END_AREA),
        ("FAILURE_AREA", *FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_FAILURE_AREA),
    ];
    let on: Vec<&str> = area_flags
        .iter()
        .filter(|(_, f)| WorkModule::is_flag(boma, *f))
        .map(|(n, _)| *n)
        .collect();
    if !on.is_empty() && !AREA_FLAG_SEEN[entry] {
        AREA_FLAG_SEEN[entry] = true;
        crate::telemetry::sink::crumb(format!(
            "[rat] WHIP p{entry} NATIVE AREA FLAGS ARE LIVE: {} on frame {} of the throw. The \
             engine is raising them from special_s_lariat_offset_x itself -- every hand-derived \
             band and ROPE_DISTANCE in this file can be deleted in favour of reading these.",
            on.join("+"),
            SINCE_THROW[entry]
        ));
    }
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
    //
    // GATED ON HAVING TURNED AROUND, not on a frame count. `RETURN_GUARD_FRAMES` was a guess at
    // how long the outbound leg lasts, needed because the old driver flew outbound on a timer.
    // Now that the turn happens when the victim reaches the ropes, "are they coming back" is a
    // fact -- and the bands at -24 and -10 only mean anything on the return.
    let gap = victim_gap(boma);
    if followup_pressed(boma) {
        let in_lariat_band = RETURNING[entry]
            && gap.map_or(false, |g| g <= LARIAT_X && g >= LARIAT_END_X);
        let next = if in_lariat_band {
            RAT_STATUS_KIND_SPECIAL_S_LARIAT
        } else {
            RAT_STATUS_KIND_SPECIAL_S_SHOULDER
        };
        fighter.change_status(next.into(), false.into());
        return 1.into();
    }

    // No press, and the rebound has arrived at `special_s_failure_offset_x`. Also gated on the
    // return, so it cannot fire while the victim is still at zero distance being held.
    if RETURNING[entry] {
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

/// How fast the lariat animation plays, between `special_s_lariat_motion_rate_min` and `_max`.
///
/// THE LARIAT HAS TO CATCH A MOVING TARGET, and its animation speed is how it does that. The
/// source scales the rate by how far the victim still had to travel when the button went in:
///
///   push   = WorkModule::get_float(FLOAT_SPECIAL_S_LARIAT_BUTTON_PUSH_POS_X)
///   hit    = WorkModule::get_float(FLOAT_SPECIAL_S_LARIAT_AREA_HIT_POS_X)
///   length = WorkModule::get_float(FLOAT_SPECIAL_S_LARIAT_AREA_LENGTH)
///   t      = |push - hit| / length          (when length > 0)
///   MotionModule::set_rate(min + (max - min) * t)
///
/// Press late, with them nearly on top of him, and it plays at 1x; press at the far edge of the
/// window and it plays at 2x so the hitbox still arrives in time. Without it the lariat is a fixed
/// 1x and an early press whiffs.
///
/// THOSE THREE WORK FLOATS CANNOT BE READ HERE, which is why this computes the same quantity from
/// the geometry instead. They are `FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLOAT_*` at indices 0x50, 0x51
/// and 0x52, and the COMMON float works end at `FIGHTER_INSTANCE_WORK_ID_FLOAT_TERM` = 0x4C, so
/// they sit past the end of Pikachu's array -- the same wall the rope flag hits in ADR 0004.
///
/// The substitution is exact rather than approximate, because this port already models the band
/// those floats describe: `special_s_lariat_end_offset_x1` 10 is where the area's hit point is and
/// `special_s_lariat_offset_x1` 24 is its far edge, so the victim's own distance out IS
/// `|push - hit|` and the band's width IS `length`.
unsafe fn lariat_rate(boma: *mut BattleObjectModuleAccessor) -> f32 {
    let Some(gap) = victim_gap(boma) else { return LARIAT_RATE_MIN };
    let span = LARIAT_X - LARIAT_END_X;
    if span <= 0.0 {
        return LARIAT_RATE_MIN;
    }
    let t = ((gap - LARIAT_END_X) / span).clamp(0.0, 1.0);
    LARIAT_RATE_MIN + (LARIAT_RATE_MAX - LARIAT_RATE_MIN) * t
}

unsafe extern "C" fn pikachu_specialslariat_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let rate = lariat_rate(boma);
    MotionModule::change_motion(
        boma, Hash40::new(&situated(fighter, "_lariat")), 0.0, 1.0, false, 0.0, false, false
    );
    // Separately, exactly as the source does it: `change_motion` at 1x and then `set_rate` with
    // the computed value. Folding it into change_motion's rate argument would be equivalent
    // today and would stop matching the moment anything else touches the rate.
    MotionModule::set_rate(boma, rate);
    crate::telemetry::probe::event(format!(
        "[rat] WHIP p{} LARIAT at rate {rate:.2} (min {LARIAT_RATE_MIN} max {LARIAT_RATE_MAX}, \
         victim {} units out); {}",
        entry_of(boma),
        victim_gap(boma).map_or(-1.0, |g| g),
        crate::telemetry::probe::ctx(boma)
    ));
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
    // The move is over, so the victim is not ours any more. The 170-frame ceiling in
    // crate::victim would get there eventually; saying so explicitly means the log reads as an
    // event rather than as a timeout.
    crate::victim::release(entry_of(boma), "side-B ended");
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
