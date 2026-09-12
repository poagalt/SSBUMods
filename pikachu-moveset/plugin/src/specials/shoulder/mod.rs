// DK's cargo carry as REAL STATUSES, which is how he does it.
//
// WHY THIS REPLACES THE MOTION-SWAP CARRY IN ../cargo.
//
// The previous design kept Pikachu in his own WAIT / WALK / TURN and only overrode the motion to
// DK's `shoulder_*` pose, so that his locomotion, speeds and jump would be inherited for free.
// Tracing the real Donkey Kong killed that idea: he is never in WAIT or WALK while carrying.
//
//     CATCH_PULL(0x3a) -> 0x1EA on throw_f
//     0x1EA -> 0x1EB   on frame 15 of throw_f
//     0x1EB <-> 0x1EC  shoulder_wait / shoulder_walk_middle
//     0x1EB <-> 0x1ED  shoulder_wait / shoulder_turn
//
// and every one of the reported symptoms falls out of the difference:
//
//   "as soon as pikachu starts moving the carry animation ends" -- WALK re-picks its own motion
//   every frame, so it overwrote the pose immediately.
//
//   "he moves with unbounded speed" -- WALK runs on CONTROL kinetic energy, which kept
//   accelerating while the shoulder animation ALSO carried root motion. DK's SHOULDER_WALK is
//   FIGHTER_KINETIC_TYPE_MOTION_LOOP: the animation moves him and there is no control energy to
//   run away with.
//
//   "none of the throws actually work" -- the cargo throw ACMDs use
//   ATTACK_ABS(FIGHTER_ATTACK_ABSOLUTE_KIND_THROW), which reads
//   FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT. Only a throw STATUS fills that in, so changing
//   motion to `throw_f_f` played an animation and nothing else.
//
//   "mario gets stuck to pikachu's feet" -- nothing was positioning the victim at all.
//
// EVERY init_settings BELOW IS DK'S, read out of his decompiled `pre` handlers in
// vendor/ssbu-status/.../lua2cpp_donkey.nro/donkey/donkey/status/Shoulder*/pre.c.
//
// THE VICTIM IS DRIVEN BY HAND, not by SHOULDERED_DONKEY. Round 3 established that requesting
// those statuses is the leading suspect for the freeze -- their code runs on the victim and is
// written for a carrier that is Donkey Kong. So their position is set directly from Pikachu's
// shoulder joint each frame, which is what the whip module already does for its own victim and
// has never crashed. What is lost is their pose: they hang upright rather than sitting.
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

/// Frame of `throw_f` at which DK hands off from SHOULDER_START to SHOULDER_WAIT.
///
/// Measured from the real fighter rather than assumed: the trace shows
/// `0x1EA -> 0x1EB on frame 15 of throw_f (anim end 24)`. His ACMD raises
/// FIGHTER_DONKEY_STATUS_SHOULDER_START_FLAG_SHOULDER_WAIT and his C++ turns that into the
/// transition; we have his flag in the ported script but nothing reads it, so the frame is used.
const SHOULDER_WAIT_FRAME: f32 = 15.0;

/// Stick deflection needed to walk or to turn, from `common/walk_stick_x` and
/// `common/turn_stick_x` -- the same commons DK's ShoulderWait reads.
const WALK_STICK_X: &str = "walk_stick_x";
const TURN_STICK_X: &str = "turn_stick_x";

/// The carried fighter's battle object id, per entry.
///
/// THE ID FORMAT IS MEASURED, NOT GUESSED. Three features were blocked on turning an entry id
/// into a battle object id, and rather than guess, the telemetry logged what the engine itself
/// hands back during Pikachu's ordinary grab. The answer: `captured object id = 0x10000` while we
/// were entry 0 and the victim was entry 1. So a fighter's object id is `entry << 16`, not the
/// `0x5000000n` the invalid sentinel suggested. Kept here as the id directly, since that is what
/// `sv_battle_object` wants.
static mut VICTIM: [u32; 8] = [0; 8];
static mut CARRY_FRAMES: [i32; 8] = [0; 8];

unsafe fn entry_of(boma: *mut BattleObjectModuleAccessor) -> usize {
    let e = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if e >= 8 { 0 } else { e }
}

/// VALIDATED, because a pointer that is not an accessor is the worst crash this project has had.
///
/// The old version accepted anything that was not the invalid sentinel and not `is_null`. That
/// leaves a stale id -- `CatchModule::capture_object_id` read a frame too late -- resolving to a
/// pointer the engine will happily make a virtual call through, and an Instruction Abort at a
/// garbage address names no function and no line. `crate::victim::boma` additionally requires the
/// accessor to agree about its own `battle_object_id`, which a stale or recycled pointer will not.
unsafe fn victim_boma(entry: usize) -> *mut BattleObjectModuleAccessor {
    crate::victim::boma(VICTIM[entry]).unwrap_or(std::ptr::null_mut())
}

/// Put the carried fighter on Pikachu's shoulder, every frame.
///
/// Reads the joint position rather than offsetting from the fighter's origin, so the victim rides
/// the animation instead of floating at a fixed height -- which is the difference between being
/// carried and being attached to a point in space.
unsafe fn hold_victim(boma: *mut BattleObjectModuleAccessor, entry: usize) {
    let vboma = victim_boma(entry);
    if vboma.is_null() {
        return;
    }
    // WHAT THE ENGINE IS DOING TO THEM, one line per change. The carry ends on THEIR side --
    // `CaptureWaitCommon` escapes when `ControlModule::get_clatter_time` reaches zero -- so the
    // clatter is the measurement that separates "never started" from "ran out". See
    // src/victim.rs.
    crate::victim::trace(entry, vboma, "cargo");
    // ONE CRUMB PER CARRY, NOT PER FRAME. The cargo crash is an Instruction Abort with no
    // symbol, so the only way to know whether control reached this function at all is for the
    // log to say so -- but a line every frame at 60fps would bury everything else. So it is
    // written on the first frame of each carry, which is also the frame the crash happens on.
    if CARRY_FRAMES[entry] <= 1 {
        crate::telemetry::sink::crumb(format!(
            "[rat] SHOULDER p{entry} hold_victim: victim {:#x} validated, about to read joint \
             'shoulderl' and write their posture",
            VICTIM[entry]
        ));
    }
    // IF THE ENGINE IS STILL HOLDING THEM, DO NOTHING. THIS IS THE SAFEST VERSION OF THIS
    // FUNCTION AND ALSO THE MOST FAITHFUL ONE.
    //
    // The cargo grab is an ordinary capture that was re-caught rather than released -- DK's
    // game_throwf uses ATTACK_ABS(FIGHTER_ATTACK_ABSOLUTE_KIND_CATCH). So while the capture link
    // survives, the engine is already placing the victim every frame: `status_CapturePulled` in
    // lua2cpp_common.nro calls `CaptureModule::update_node_pos`, `motion` and `motion_lw` and
    // nothing else -- no LinkModule parent reads, nothing fighter-specific, and Pikachu has the
    // `Throw` bone it positions against.
    //
    // Writing their posture on top of that is two writers fighting over one transform, and it is
    // the only cross-object write in the whole carry -- which makes it the leading candidate for
    // an Instruction Abort with no symbol. So it is now the FALLBACK, taken only once the engine
    // has let go, and the log says which of the two is driving.
    if CaptureModule::is_capture(vboma)
        && CaptureModule::catch_object_id(vboma) as u32 == (*boma).battle_object_id
    {
        if CARRY_FRAMES[entry] <= 1 {
            crate::telemetry::sink::crumb(format!(
                "[rat] SHOULDER p{entry} victim {:#x} is still in the engine's capture and \
                 naming us as master -- leaving their position to CaptureModule",
                VICTIM[entry]
            ));
        }
        return;
    }

    let mut pos = Vector3f { x: 0.0, y: 0.0, z: 0.0 };
    ModelModule::joint_global_position(boma, Hash40::new("shoulderl"), &mut pos, true);
    // A joint that did not resolve returns the origin; falling back to the fighter's own position
    // plus a height is better than dropping the victim through the floor.
    if pos.x == 0.0 && pos.y == 0.0 {
        pos.x = PostureModule::pos_x(boma);
        pos.y = PostureModule::pos_y(boma) + 8.0;
    }
    PostureModule::set_pos(vboma, &pos);
    // Their own momentum must not fight the placement, or they drift off the shoulder.
    KineticModule::clear_speed_all(vboma);
}

/// True while the victim is still ours to carry.
///
/// THE CAPTURE LINK COMES FIRST, AND THE STATUS LIST IS THE FALLBACK. Keying this on the victim's
/// status alone was fragile in a way that shows up as "the carry does nothing": the engine's own
/// THROW ends when our SHOULDER_START takes over, so the victim leaves THROWN within a frame or
/// two and every status in the list stops matching. `CaptureModule::catch_object_id` naming us is
/// the durable fact, and it is the same question `hold_victim` asks.
unsafe fn still_held(boma: *mut BattleObjectModuleAccessor, entry: usize) -> bool {
    // OURS TO ANSWER. "The opponent falls off Pikachu's shoulder after a second" is this
    // function, and the reason is the same one that cancelled the whip's rope throw: it was
    // asking the ENGINE whether we were still holding someone.
    //
    // The cargo grab is an ordinary capture that DK's game_throwf re-catches rather than
    // releases -- but the engine's own THROW status ends the moment SHOULDER_START takes over,
    // and with it the capture link and every CAPTURE_* status. So every test here went false
    // within a frame or two of the carry starting, and the carry dropped the victim.
    //
    // `crate::victim::held` is the fact, with a 170-frame ceiling so it cannot become a stuck
    // grab. The engine's link is still accepted where it exists -- `hold_victim` leans on it to
    // do the positioning for us -- but its absence proves nothing.
    let _ = boma;
    crate::victim::held(entry).is_some()
}

/// Common `pre` for the grounded carry states. DK uses the same shape for all of them and varies
/// only the kinetic type and the ground correction.
/// The carry status settings, which are Donkey Kong's down to the last argument.
///
/// `FS_SUCCEEDS_KEEP_ATTACK_ABSOLUTE` IS THE ONE THAT WAS MISSING, and all eleven of his carry
/// statuses pass it -- ShoulderStart, Wait, Walk, Turn, Fall, Landing, Pass and the four jumps.
/// This port passed 0.
///
/// The measurement says what that costs. `crate::victim::trace` logged the carried fighter as
/// `status=0x44 clatter=65.0` on the first frame of the carry and `status=0xe` two lines later:
/// 0x44 is FIGHTER_STATUS_KIND_CAPTURE_CUT and 0xe is FIGHTER_STATUS_KIND_FALL. The victim was
/// ALREADY CUT LOOSE before the carry loop ran once, and the clatter counting 65 -> 31 -> 0 is the
/// tail of an escape that had already been decided. Every playtest report of the victim "falling
/// off his shoulder" was describing a fighter who was never held.
///
/// The attack-absolute data is the throw relationship -- it is what
/// FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT is read against -- and entering a status without
/// this flag discards it, so the engine has no reason to keep the capture alive.
unsafe fn shoulder_pre(
    fighter: &mut L2CFighterCommon,
    kinetic: i32,
    ground_correct: i32,
) -> L2CValue {
    StatusModule::init_settings(
        fighter.module_accessor,
        smash::app::SituationKind(*SITUATION_KIND_NONE),
        kinetic,
        ground_correct as u32,
        smash::app::GroundCliffCheckKind(*GROUND_CLIFF_CHECK_KIND_NONE),
        true,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLAG,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_INT,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLOAT,
        *FS_SUCCEEDS_KEEP_ATTACK_ABSOLUTE,
    );
    FighterStatusModuleImpl::set_fighter_status_data(
        fighter.module_accessor,
        // His first argument is `true` here and `false` in the throws. Both copied rather than
        // chosen.
        true,
        *FIGHTER_TREADED_KIND_NO_REAC,
        false,
        false,
        false,
        0,
        0,
        0,
        0,
    );
    0.into()
}

// ---- SHOULDER_START : the lift, out of throw_f ----------------------------------------------

unsafe extern "C" fn shoulder_start_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(fighter, *FIGHTER_KINETIC_TYPE_MOTION, *GROUND_CORRECT_KIND_GROUND_OTTOTTO)
}

unsafe extern "C" fn shoulder_start_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] = 0;
    // CRUMBED, because the cargo crash lands here and the report cannot say why. The Atmosphere
    // backtrace for it is `rat_of_doom + 0x9cf8` under smashline own line hook with no engine
    // frame between -- an Instruction Abort, so a branch to an address that is not code. That
    // rules in a bad function or object pointer and rules out a null field read, and it names
    // nothing else. These two lines bracket the status handoff so the log says which side of it
    // the game died on.
    crate::telemetry::sink::crumb(format!(
        "[rat] SHOULDER p{entry} START entered, victim={:#x}, about to shift to the carry loop",
        VICTIM[entry]
    ));
    crate::telemetry::probe::event(format!(
        "[rat] SHOULDER p{entry} START, victim={:#x}; {}",
        VICTIM[entry],
        crate::telemetry::probe::ctx(boma)
    ));
    // DK stays on `throw_f` through the lift; the pose only changes at SHOULDER_WAIT.
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_start_loop as *const () as _))
}

/// ON THE SHOULDER, NOT AT THE FEET -- the second attempt at `SHOULDERED_DONKEY`, one frame later
/// and with a precondition.
///
/// WHY IT CRASHED THE FIRST TIME, from the report rather than from a guess. Data Abort on
/// MainThread at `main + 0x484da4`:
///
///     plVar8 = *(long **)(lVar9 + lVar7 * 0x68 + 0x58);   // a LINK SLOT field
///     ...
///     uVar5 = (**(code **)(*plVar8 + 0x3c8))(plVar8);     // virtual call, plVar8 == null
///
/// `0x68` is the link-slot stride -- the same stride `LinkModule::link` indexes with at
/// main + 0x47f120 -- so `+0x58` is a field of the capture link slot, holding a resolved OBJECT
/// POINTER. And `Link` does not write it: it writes the parent at `+0x30` and the id at `+0x48`
/// and nothing else. The engine fills the cached pointer in itself as part of a real catch, and
/// the port forced the status on the SAME FRAME as the link, before it existed.
///
/// So this waits a frame and refuses unless the slot looks resolved: the node list must name our
/// victim, which means the engine has walked the link at least once. That is a positive
/// precondition rather than a delay in the hope it is enough.
///
/// WHY BOTHER, when `docs/the-recording.md` says the real victim plays `capture_wait`. Because
/// `CAPTURE_WAIT` positions them where an ORDINARY GRAB holds them, and the playtest says exactly
/// that: "moving while carrying mario causes him to get stuck to pikachu's feet". The recording is
/// right about the motion and says nothing about the node; the shoulder pose lives in
/// `SHOULDERED_DONKEY`, which is why the whole family exists.
unsafe fn shoulder_the_victim(boma: *mut BattleObjectModuleAccessor, entry: usize) {
    if !SHOULDER_THE_VICTIM || CARRY_FRAMES[entry] != 2 {
        return;
    }
    let them = VICTIM[entry];
    let Some(vboma) = crate::victim::boma(them) else {
        return;
    };
    // The precondition: the engine has resolved the link far enough to name our victim through it.
    if LinkModule::get_node_object_id(boma, *LINK_NO_CAPTURE) as u32 != them {
        crate::telemetry::sink::diag(format!(
            "[rat] SHOULDER p{entry} NOT shouldering {them:#x}: the capture link's node does not \
             name them yet, so main+0x484da4's cached object pointer would still be null"
        ));
        return;
    }
    crate::telemetry::sink::diag(format!(
        "[rat] SHOULDER p{entry} shouldering {them:#x} on carry frame 2 (their st={:#x})",
        StatusModule::status_kind(vboma)
    ));
    crate::victim::request_status(
        vboma,
        *FIGHTER_STATUS_KIND_SHOULDERED_DONKEY_START,
        "cargo shoulder",
    );
}

/// Flip to false to go back to leaving the victim in `CAPTURE_WAIT`. Kept as a switch because this
/// is the line that crashed once already, and a playtest that crashes on it should cost one edit.
const SHOULDER_THE_VICTIM: bool = true;

unsafe extern "C" fn shoulder_start_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    // THE SECOND HALF OF THE CARGO CRASH BRACKET. shoulder_start_main writes a crumb saying it is
    // about to shift into this loop; this one says the loop was reached. If the log ends on the
    // first crumb and never shows the second, the game died inside sub_shift_status_main -- which
    // is what the Instruction Abort backtrace suggests and what no symbol in it can confirm.
    if CARRY_FRAMES[entry] == 1 {
        crate::telemetry::sink::crumb(format!(
            "[rat] SHOULDER p{entry} START loop reached, frame 1"
        ));
    }
    shoulder_the_victim(boma, entry);
    hold_victim(boma, entry);

    if !still_held(boma, entry) {
        apply_lift_param(boma, entry, false);
        return carry_change(fighter, entry, *FIGHTER_STATUS_KIND_WAIT, "victim lost -> WAIT");
    }
    if MotionModule::frame(boma) >= SHOULDER_WAIT_FRAME || MotionModule::is_end(boma) {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_WAIT, "SHOULDER_WAIT");
    }
    0.into()
}

// ---- SHOULDER_WAIT : the hold, and the decision point ----------------------------------------

unsafe extern "C" fn shoulder_wait_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(fighter, *FIGHTER_KINETIC_TYPE_MOTION, *GROUND_CORRECT_KIND_GROUND_OTTOTTO)
}

unsafe extern "C" fn shoulder_wait_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    crate::telemetry::sink::crumb(format!(
        "[rat] SHOULDER p{} WAIT entered, about to change motion to shoulder_wait",
        entry_of(boma)
    ));
    MotionModule::change_motion(boma, Hash40::new("shoulder_wait"), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_wait_loop as *const () as _))
}

/// The throw the input asks for, if any.
///
/// DK'S OWN CHECK, READ OUT OF THE BINARY. `FUN_7100022520` is the helper every Shoulder loop
/// calls, and the public dump omits it because it could not name it. `tools/disasm` resolves the
/// NRO's PLT against its .dynsym, so it reads plainly now:
///
///   LinkModule::get_node_object_id      the fighter on his shoulder
///   ControlModule::get_clatter_time     on the VICTIM; `<= 0` returns early with no throw
///   globalTable[0x21] & <four consecutive constants>     cat2, four bits apart
///   L2CFighterCommon::IsThrowStick
///   globalTable[0x20] & <an OR of four more>             cat1
///   five change_status calls, chosen by a hash-indexed lookup
///
/// `globalTable[0x20]` is cat1 and `[0x21]` is cat2 -- ShoulderStart/main_loop.c and
/// SpecialNLoop/main_loop.c in the same dump index them with named CAT1 and CAT2 constants, which
/// is what fixes the two indices. The four consecutive cat2 constants can only be
/// FIGHTER_PAD_CMD_CAT2_FLAG_THROW_F, _B, _HI and _LW: adjacent slots in the constant table, and
/// bits 0x100000, 0x200000, 0x400000, 0x800000 -- the four throw commands, adjacent in the
/// enumeration too.
///
/// SO THE DIRECTION IS THE ENGINE'S, NOT A THRESHOLD OF OURS. This used to compare
/// `get_stick_x` and `get_stick_y` against 0.5. The cat2 throw commands are the same thing the
/// game resolves for an ordinary grab, which means smash-stick deflection, the input buffer and
/// the per-controller deadzones all behave here exactly as they do out of a normal grab -- none
/// of which a raw stick read reproduces.
///
/// The button path survives as the second half of DK's own test, `IsThrowStick` together with a
/// cat1 button mask, because that is what he does when the command flag has not fired. TRIGGER
/// rather than held: the grab button is still down when the carry begins, and reading it as held
/// is what committed a throw on the first carry frame and froze the game in round 2.
unsafe fn requested_throw(fighter: &mut L2CFighterCommon) -> Option<i32> {
    let boma = fighter.module_accessor;

    // THE BUTTON IS THE TRIGGER. THE STICK ONLY CHOOSES THE DIRECTION.
    //
    // This had it the other way round: a cat2 throw flag on its own committed a throw. Those
    // flags fire on a STICK FLICK -- that is what a "throw command" is in the input system -- so
    // walking while carrying threw the victim. Playtest: "modified pikachu cannot move while
    // cargo throwing without actually throwing", and the previous log agrees, showing
    // SHOULDER_WAIT lasting six frames before a THROW_F_LW.
    //
    // That contradicted the actual move and it also contradicted DK's own helper, which I had
    // read too loosely. `FUN_7100022520` computes FOUR cat2 masks and ONE cat1 mask and combines
    // them -- five constants OR'd into the cat1 mask, then an `operator&` against
    // globalTable[0x20]. A single `and` against cat1 in the decision means the BUTTON is
    // mandatory; the four cat2 tests are how the direction is picked once it is pressed. The
    // cat1 mask being five flags wide is consistent with "any attack input".
    //
    // So: no press, no throw. Then the direction comes from the cat2 command flags if the input
    // system resolved one, and from the raw stick otherwise -- the command flags are better
    // because they account for smash-stick deflection, buffering and per-controller deadzones,
    // and the raw read is the fallback for an attack pressed with the stick already held.
    let pressed = ControlModule::check_button_trigger(boma, *CONTROL_PAD_BUTTON_ATTACK)
        || ControlModule::check_button_trigger(boma, *CONTROL_PAD_BUTTON_SPECIAL);
    if !pressed {
        return None;
    }

    let cat2 = ControlModule::get_command_flag_cat(boma, 1);
    if cat2 & *FIGHTER_PAD_CMD_CAT2_FLAG_THROW_HI != 0 {
        return Some(RAT_STATUS_KIND_THROW_F_HI);
    }
    if cat2 & *FIGHTER_PAD_CMD_CAT2_FLAG_THROW_LW != 0 {
        return Some(RAT_STATUS_KIND_THROW_F_LW);
    }
    if cat2 & *FIGHTER_PAD_CMD_CAT2_FLAG_THROW_B != 0 {
        return Some(RAT_STATUS_KIND_THROW_F_B);
    }
    if cat2 & *FIGHTER_PAD_CMD_CAT2_FLAG_THROW_F != 0 {
        return Some(RAT_STATUS_KIND_THROW_F_F);
    }
    // `IsThrowStick` DECIDES THE DIRECTION, IT DOES NOT GATE THE THROW. A cargo attack press with
    // the stick neutral is a forward throw in the real move, so requiring a deflection here would
    // remove the most common input in the move. DK's helper reads it alongside the cat1 button
    // mask for the same reason: to know whether a direction was asked for.
    let x = if fighter.IsThrowStick().get_bool() {
        ControlModule::get_stick_x(boma) * PostureModule::lr(boma)
    } else {
        0.0
    };
    let y = ControlModule::get_stick_y(boma);
    if y > 0.5 {
        Some(RAT_STATUS_KIND_THROW_F_HI)
    } else if y < -0.5 {
        Some(RAT_STATUS_KIND_THROW_F_LW)
    } else if x < -0.5 {
        Some(RAT_STATUS_KIND_THROW_F_B)
    } else {
        Some(RAT_STATUS_KIND_THROW_F_F)
    }
}

unsafe extern "C" fn shoulder_wait_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    hold_victim(boma, entry);

    if !still_held(boma, entry) {
        crate::telemetry::probe::event(format!(
            "[rat] SHOULDER p{entry} victim lost after {}f; {}",
            CARRY_FRAMES[entry],
            crate::telemetry::probe::ctx(boma)
        ));
        apply_lift_param(boma, entry, false);
        return carry_change(fighter, entry, *FIGHTER_STATUS_KIND_WAIT, "victim lost -> WAIT");
    }
    if let Some(next) = requested_throw(fighter) {
        crate::telemetry::probe::event(format!(
            "[rat] SHOULDER p{entry} throw {} after {}f; {}",
            crate::telemetry::probe::st(next),
            CARRY_FRAMES[entry],
            crate::telemetry::probe::ctx(boma)
        ));
        return carry_change(fighter, entry, next, "requested throw");
    }
    // Airborne -- walked off a ledge.
    if StatusModule::situation_kind(boma) != *SITUATION_KIND_GROUND {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_FALL, "SHOULDER_FALL");
    }

    // DK's ShoulderWait reads `common/turn_stick_x` and `common/walk_stick_x` in that order, and
    // turn wins -- the stick pointing behind you is a turn, not a backwards walk.
    let stick_x = ControlModule::get_stick_x(boma);
    let lr = PostureModule::lr(boma);
    let turn_min = WorkModule::get_param_float(boma, smash::hash40(TURN_STICK_X), 0);
    let walk_min = WorkModule::get_param_float(boma, smash::hash40(WALK_STICK_X), 0);
    if stick_x * lr < -turn_min.max(0.1) {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_TURN, "SHOULDER_TURN");
    }
    if stick_x.abs() > walk_min.max(0.1) {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_WALK, "SHOULDER_WALK");
    }
    0.into()
}

// ---- SHOULDER_WALK ---------------------------------------------------------------------------

unsafe extern "C" fn shoulder_walk_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    // MOTION_LOOP: the animation moves him. This is the fix for the unbounded speed.
    shoulder_pre(fighter, *FIGHTER_KINETIC_TYPE_MOTION_LOOP, *GROUND_CORRECT_KIND_GROUND_OTTOTTO)
}

/// DK's `SHOULDER_WALK` `INIT_STATUS`, donkey `0x7100008650`, which is ONE CALL:
///
///     sub_walk_uniq_process_init_common("shoulder_walk_fast", "shoulder_walk_middle",
///                                       "shoulder_walk_slow", 0)
///
/// THIS REPLACES A HAND-ROLLED THREE-WAY POSE PICK. `shoulder_walk_main` used to read
/// `ControlModule::get_stick_x` and choose between the three motions at thresholds of 0.85 and
/// 0.50, which are numbers from nowhere -- not DK's, not Pikachu's, not any prc. The engine has a
/// three-speed walk built in; `sub_walk_uniq_process_init_common` hands it the three motion names
/// and it does the rest from the FIGHTER'S OWN walk params, which on a Pikachu means Pikachu's
/// walk thresholds and Pikachu's speeds -- the same relationship every other attribute in this
/// moveset has to its source.
///
/// The argument ORDER is fast, middle, slow, and it was checked rather than assumed: the raw
/// hashes in DK's function are 0x127d758abd, 0x1462101369, 0x126d14b2a4 and `tools/hash40.sh`
/// resolves them in that order to `shoulder_walk_fast`, `_middle`, `_slow`. Two of the three are
/// eighteen characters, so the length byte alone could not have told fast from slow.
///
/// His `EXEC_STATUS` for this status (`0x7100006170`) contains no calls at all -- the per-frame
/// drive is entirely the common walk code, which is the point of routing through it.
unsafe extern "C" fn shoulder_walk_init(fighter: &mut L2CFighterCommon) -> L2CValue {
    fighter.sub_walk_uniq_process_init_common(
        L2CValue::new_int(Hash40::new("shoulder_walk_fast").hash),
        L2CValue::new_int(Hash40::new("shoulder_walk_middle").hash),
        L2CValue::new_int(Hash40::new("shoulder_walk_slow").hash),
        L2CValue::new_int(0),
    );
    0.into()
}

unsafe extern "C" fn shoulder_walk_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    // No motion change here any more: `shoulder_walk_init` handed the three names to the engine's
    // own walk process, which owns both the pose and the speed from this point.
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_walk_loop as *const () as _))
}

unsafe extern "C" fn shoulder_walk_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    hold_victim(boma, entry);

    if !still_held(boma, entry) {
        apply_lift_param(boma, entry, false);
        return carry_change(fighter, entry, *FIGHTER_STATUS_KIND_WAIT, "victim lost -> WAIT");
    }
    if let Some(next) = requested_throw(fighter) {
        return carry_change(fighter, entry, next, "requested throw");
    }
    if StatusModule::situation_kind(boma) != *SITUATION_KIND_GROUND {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_FALL, "SHOULDER_FALL");
    }
    let stick_x = ControlModule::get_stick_x(boma);
    let lr = PostureModule::lr(boma);
    if stick_x * lr < -0.1 {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_TURN, "SHOULDER_TURN");
    }
    if stick_x.abs() <= 0.1 {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_WAIT, "SHOULDER_WAIT");
    }
    0.into()
}

// ---- SHOULDER_TURN ---------------------------------------------------------------------------

unsafe extern "C" fn shoulder_turn_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(fighter, *FIGHTER_KINETIC_TYPE_TURN, *GROUND_CORRECT_KIND_GROUND)
}

unsafe extern "C" fn shoulder_turn_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    MotionModule::change_motion(boma, Hash40::new("shoulder_turn"), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_turn_loop as *const () as _))
}

unsafe extern "C" fn shoulder_turn_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    hold_victim(boma, entry);

    if !still_held(boma, entry) {
        apply_lift_param(boma, entry, false);
        return carry_change(fighter, entry, *FIGHTER_STATUS_KIND_WAIT, "victim lost -> WAIT");
    }
    if MotionModule::is_end(boma) {
        // The turn is what flips him; doing it at the end matches DK, whose animation carries the
        // rotation and whose status only commits it once.
        PostureModule::reverse_lr(boma);
        PostureModule::update_rot_y_lr(boma);
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_WAIT, "SHOULDER_WAIT");
    }
    0.into()
}

// ---- SHOULDER_FALL / _LANDING ----------------------------------------------------------------

unsafe extern "C" fn shoulder_fall_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(fighter, *FIGHTER_KINETIC_TYPE_MOTION_FALL, *GROUND_CORRECT_KIND_AIR)
}

unsafe extern "C" fn shoulder_fall_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    MotionModule::change_motion(boma, Hash40::new("shoulder_fall"), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_fall_loop as *const () as _))
}

unsafe extern "C" fn shoulder_fall_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    hold_victim(boma, entry);

    if !still_held(boma, entry) {
        return carry_change(fighter, entry, *FIGHTER_STATUS_KIND_FALL, "victim lost -> FALL");
    }
    if let Some(next) = requested_throw(fighter) {
        return carry_change(fighter, entry, next, "requested throw");
    }
    if StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_LANDING, "SHOULDER_LANDING");
    }
    0.into()
}

unsafe extern "C" fn shoulder_landing_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(fighter, *FIGHTER_KINETIC_TYPE_MOTION, *GROUND_CORRECT_KIND_GROUND_OTTOTTO)
}

unsafe extern "C" fn shoulder_landing_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    MotionModule::change_motion(boma, Hash40::new("shoulder_landing"), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_landing_loop as *const () as _))
}

unsafe extern "C" fn shoulder_landing_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    hold_victim(boma, entry);

    if !still_held(boma, entry) {
        apply_lift_param(boma, entry, false);
        return carry_change(fighter, entry, *FIGHTER_STATUS_KIND_WAIT, "victim lost -> WAIT");
    }
    if MotionModule::is_end(boma) {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_WAIT, "SHOULDER_WAIT");
    }
    0.into()
}

// ---- THE FOUR CARGO THROWS -------------------------------------------------------------------

/// Common `pre` for the throws, matching the engine's own THROW status shape.
// THE THROW STATUS SETTINGS ARE DONKEY KONG'S, ARGUMENT FOR ARGUMENT, AND THE KEEP FLAGS ARE
// WHY THIS MATTERS. `FIGHTER_STATUS_WORK_KEEP_FLAG_THROW_INT` is what preserves the THROW status
// work ints across the transition into the throw -- and
// FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT, the id of the fighter being thrown, is one of
// them. This passed `NONE_INT`, which tells the engine to CLEAR every status work int on entry,
// so the throw began with no record of who it was throwing.
//
// The kinetic type is his too: UNIQ, not MOTION. His four cargo throws call no kinetic or speed
// function at all -- `CatchModule::catch_cut`, `set_fighter_status_data`, `init_settings`,
// `get_node_object_id`, `send_event_nodes_throw`, `get_int64` and `set_int` is the whole list --
// so he plants and throws rather than sliding, and MOTION was letting the animation's root motion
// move him.
unsafe extern "C" fn throw_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    StatusModule::init_settings(
        fighter.module_accessor,
        smash::app::SituationKind(*SITUATION_KIND_NONE),
        *FIGHTER_KINETIC_TYPE_UNIQ,
        *GROUND_CORRECT_KIND_KEEP as u32,
        smash::app::GroundCliffCheckKind(*GROUND_CLIFF_CHECK_KIND_NONE),
        true,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_THROW_FLAG,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_THROW_INT,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_THROW_FLOAT,
        0,
    );
    FighterStatusModuleImpl::set_fighter_status_data(
        fighter.module_accessor,
        false,
        *FIGHTER_TREADED_KIND_NO_REAC,
        false,
        false,
        false,
        (*FIGHTER_LOG_MASK_FLAG_ACTION_CATEGORY_CATCH | *FIGHTER_LOG_MASK_FLAG_ACTION_TRIGGER_ON) as u64,
        0,
        0,
        0,
    );
    0.into()
}

/// Start one of the four throws.
///
/// THE TARGET WORK VARIABLES ARE THE POINT. The cargo throw ACMDs read
/// FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT, _TARGET_HIT_GROUP and _TARGET_HIT_NO and pass them
/// to ATK_HIT_ABS. Without them the ATTACK_ABS(THROW) has nobody to throw, which is precisely why
/// changing motion alone did nothing. Filling them here is what turns the animation into a throw.
/// The victim's thrown motion for each of our four throw statuses, and the event that delivers it.
///
/// DK'S OWN LOOP, CLOSED. `ShoulderStart_exit` writes the throw's own motion name into
/// `FIGHTER_STATUS_CATCH_WAIT_WORK_INT_MOTION_KIND` (see `leave_carry`), and then `ThrowFF_init`
/// reads it straight back out and maps it to the motion the VICTIM plays:
///
///   WorkModule::get_int64(boma, FIGHTER_STATUS_CATCH_WAIT_WORK_INT_MOTION_KIND)
///     hash40("throw_f_f")  -> hash40("thrown_f_donkey_f")
///     hash40("throw_f_b")  -> hash40("thrown_f_donkey_b")
///     hash40("throw_f_hi") -> hash40("thrown_f_donkey_hi")
///     else                 -> hash40("thrown_f_donkey_lw")
///   LinkModule::send_event_nodes_throw(boma, hash40("shoulder_throw"), that, true,
///       FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_GROUP,
///       FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_NO,
///       FIGHTER_STATUS_THROW_WORK_FLOAT_MOTION_RATE)
///
/// Every one of those five names is confirmed by hash rather than guessed -- 0x11474b27b0,
/// 0x114026e3a9, 0x12b3c9e354, 0x122daa1b33 and 0xe9b37bc21 in the decompiled `init` are exactly
/// hash40 of the five strings above.
///
/// AND THE VICTIM ALREADY HAS THE ANIMATIONS, which is what makes this free. `thrown_f_donkey_f`
/// and its three siblings ship with EVERY fighter -- `extracted/fighter/pickel/motion/body/c00/`
/// has `d05thrownfdonkeyf.nuanmb` and `d05thrownfdonkeyb.nuanmb` -- because anyone can be cargo
/// thrown by Donkey Kong. Nothing has to be shipped for this and the animation budget is untouched.
///
/// Without it the victim is launched with whatever pose they were already in, which is the other
/// half of "a spinning arms animation that looks like an animation that would happen when mario is
/// falling": the carry never told them to be carried and the throw never told them they were
/// thrown.
fn thrown_motion(throw: &str) -> &'static str {
    match throw {
        "throw_f_f" => "thrown_f_donkey_f",
        "throw_f_b" => "thrown_f_donkey_b",
        "throw_f_hi" => "thrown_f_donkey_hi",
        _ => "thrown_f_donkey_lw",
    }
}

 unsafe fn throw_main(fighter: &mut L2CFighterCommon, motion: &str) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    WorkModule::set_int64(boma, VICTIM[entry] as u64, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT);
    WorkModule::set_int64(boma, 0, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_GROUP);
    WorkModule::set_int64(boma, 0, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_NO);
    // TELL THE VICTIM THEY HAVE BEEN THROWN -- DK does this in the throw status's Init line and
    // this port never did it at all. See `thrown_motion`.
    let thrown = thrown_motion(motion);
    if linked_victim(boma, entry, "throw") {
        LinkModule::send_event_nodes_throw(
            boma,
            Hash40::new("shoulder_throw"),
            Hash40::new(thrown),
            true,
            *FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_GROUP,
            *FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_NO,
            *FIGHTER_STATUS_THROW_WORK_FLOAT_MOTION_RATE,
        );
    }
    crate::telemetry::probe::event(format!(
        "[rat] SHOULDER p{entry} throw motion '{motion}' target={:#x}; {}",
        VICTIM[entry],
        crate::telemetry::probe::ctx(boma)
    ));
    MotionModule::change_motion(boma, Hash40::new(motion), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(throw_loop as *const () as _))
}

unsafe extern "C" fn throw_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    if MotionModule::is_end(boma) {
        VICTIM[entry] = *BATTLE_OBJECT_ID_INVALID as u32;
        crate::victim::release(entry, "cargo throw finished");
        apply_lift_param(boma, entry, false);
        let ground = StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND;
        let next = if ground { *FIGHTER_STATUS_KIND_WAIT } else { *FIGHTER_STATUS_KIND_FALL };
        return carry_change(fighter, entry, next, "requested throw");
    }
    0.into()
}

unsafe extern "C" fn throw_ff_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    throw_main(fighter, "throw_f_f")
}
unsafe extern "C" fn throw_fb_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    throw_main(fighter, "throw_f_b")
}
unsafe extern "C" fn throw_fhi_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    throw_main(fighter, "throw_f_hi")
}
unsafe extern "C" fn throw_flw_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    throw_main(fighter, "throw_f_lw")
}

/// Called from ../cargo when the grab lands, to hand the carry over.
pub unsafe fn begin(fighter: &mut L2CFighterCommon, victim: u32) {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    VICTIM[entry] = victim;
    CARRY_FRAMES[entry] = 0;
    // OURS NOW. The engine releases them when its own THROW ends, so nothing downstream may ask
    // it whether the carry is still live -- see `still_held` and crate::victim.
    crate::victim::hold(entry, victim);
    crate::victim::untrace(entry);
    apply_lift_param(boma, entry, true);
    StatusModule::change_status_request_from_script(
        boma, RAT_STATUS_KIND_SHOULDER_START, false
    );
}

pub fn install() {
    // `Init` AND `Exit` ARE REGISTERED NOW, which is where DK's carry actually lives. See
    // `docs/the-carry.md`: his eleven carry statuses share one `EXIT_STATUS` and his lift setup is
    // an `INIT_STATUS`, and this module previously had ELEVEN `Main` and TEN `Pre` hooks and none
    // of these. The bodies were already right; only their line was wrong.
    Agent::new("pikachu")
        .status(Init, RAT_STATUS_KIND_SHOULDER_START,   shoulder_start_init)
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_START,   shoulder_start_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_START,   shoulder_start_main)
        .status(Exit, RAT_STATUS_KIND_SHOULDER_START,   shoulder_exit)
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_WAIT,    shoulder_wait_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_WAIT,    shoulder_wait_main)
        .status(Exit, RAT_STATUS_KIND_SHOULDER_WAIT,    shoulder_exit)
        .status(Init, RAT_STATUS_KIND_SHOULDER_WALK,    shoulder_walk_init)
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_WALK,    shoulder_walk_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_WALK,    shoulder_walk_main)
        .status(Exit, RAT_STATUS_KIND_SHOULDER_WALK,    shoulder_exit)
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_TURN,    shoulder_turn_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_TURN,    shoulder_turn_main)
        .status(Exit, RAT_STATUS_KIND_SHOULDER_TURN,    shoulder_exit)
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_FALL,    shoulder_fall_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_FALL,    shoulder_fall_main)
        .status(Exit, RAT_STATUS_KIND_SHOULDER_FALL,    shoulder_exit)
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_LANDING, shoulder_landing_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_LANDING, shoulder_landing_main)
        .status(Exit, RAT_STATUS_KIND_SHOULDER_LANDING, shoulder_exit)
        .status(Pre,  RAT_STATUS_KIND_THROW_F_F,  throw_pre)
        .status(Main, RAT_STATUS_KIND_THROW_F_F,  throw_ff_main)
        .status(Pre,  RAT_STATUS_KIND_THROW_F_B,  throw_pre)
        .status(Main, RAT_STATUS_KIND_THROW_F_B,  throw_fb_main)
        .status(Pre,  RAT_STATUS_KIND_THROW_F_HI, throw_pre)
        .status(Main, RAT_STATUS_KIND_THROW_F_HI, throw_fhi_main)
        .status(Pre,  RAT_STATUS_KIND_THROW_F_LW, throw_pre)
        .status(Main, RAT_STATUS_KIND_THROW_F_LW, throw_flw_main)
        .install();
}

/// DK'S OWN LIFT PARAMETERS, APPLIED BY DK'S OWN FUNCTION.
///
/// `donkey param_lifting_walk` is three 1.1x multipliers -- walk accel, walk accel add, walk speed
/// max -- and the header used to list them as NOT MODELLED with a reason that was entirely about
/// the difficulty of applying them by hand: applied once they are wiped by the next status change
/// that re-initialises the control energy from params, and applied every frame they compound, and
/// 1.1 per frame is Pikachu off the stage inside a second.
///
/// That problem is DK's too, and `main` exports his answer to it:
///
///     app::FighterSpecializer_Donkey::apply_lift_param(FighterModuleAccessor&, bool)
///
/// A `FighterModuleAccessor` is a `BattleObjectModuleAccessor` as its only member, so Pikachu's
/// accessor is one. The bool is on/off -- the only sensible reading of a one-argument setter whose
/// name is "apply", and it is called with `true` entering the carry and `false` leaving it.
///
/// IT WAS TRIED, IT WAS MEASURED, AND IT IS A NO-OP -- SO THE CALL IS GONE.
///
/// The note above argued the worst case was "it multiplies a param object Pikachu does not have,
/// which is a no-op, and the log says whether his walk speed changed". The log said:
///
///     [rat] SHOULDER p0 apply_lift_param(true); walk_speed_max 1.5750 -> 1.5750
///
/// Unchanged. So the call did nothing and removing it costs nothing -- `donkey
/// param_lifting_walk`'s three 1.1x multipliers are back to NOT MODELLED, which is where the
/// header had them and is the honest state.
///
/// AND THE ARGUMENT FOR TRYING IT HAD A HOLE THAT COST A PLAYTEST ELSEWHERE. It admitted two
/// outcomes, "works" or "no-op", and missed the third: a foreign fighter's native helper carries
/// that fighter's per-fighter ENUMERATIONS inside itself, so it can do something entirely
/// different on the host. `FighterSpecializer_Gaogaen::generate_rope` was justified the same way
/// in `whip/mod.rs` and it spawned a THUNDER JOLT, because Incineroar's rope index is Pikachu's
/// DENGEKIDAMA. `tools/check-status.sh` now refuses the whole `FighterSpecializer_<foreign>`
/// namespace in hand-written code rather than judging these one at a time.
///
/// The measurement is kept because it is the evidence, and because a future round that wants these
/// multipliers should start from "the native helper does nothing here" rather than re-deriving it.
unsafe fn apply_lift_param(boma: *mut BattleObjectModuleAccessor, entry: usize, on: bool) {
    let before = WorkModule::get_param_float(boma, smash::hash40("walk_speed_max"), 0);
    let after = WorkModule::get_param_float(boma, smash::hash40("walk_speed_max"), 0);
    crate::telemetry::sink::crumb(format!(
        "[rat] SHOULDER p{entry} apply_lift_param({on}); walk_speed_max {before:.4} -> {after:.4}"
    ));
}

/// Change status, unless this frame has already changed status too many times.
///
/// EVERY TRANSITION IN THE CARRY GOES THROUGH HERE. The carry has ten statuses that can reach
/// each other and a line callback that can re-enter it, and it froze the game with 7965 status
/// changes in a single frame. The specific cause is fixed in ../cargo, but the shape is available
/// to any pair of states whose conditions disagree by one frame -- and a freeze is the worst
/// failure this project can produce: no crash report, no log flush, no input, only a power cycle.
///
/// So the budget in `crate::victim` bounds it. Refusing a transition leaves the fighter in the
/// current status for the rest of the frame, which is always a legal state, and the log says once
/// that it happened.

// ---------------------------------------------------------------------------------------------
// DK'S OWN CARRY PROTOCOL, WHICH THIS PORT WAS NOT SPEAKING.
//
// Every status here reproduces DK's kinetics and motions, and the victim still slid off his
// shoulder with a tumble animation and then fell. The reason is not in the statuses at all -- it
// is five calls in `ShoulderStart_init` and `ShoulderStart_exit`, decompiled from
// lua2cpp_donkey.nro, that are the entire conversation between the carrier and the carried:
//
//   init:  LinkModule::send_event_nodes(boma, LINK_NO_CAPTURE, hash40("shoulder"), 0)
//          CaptureModule::set_ignore_catching(boma, true)          [via MA_MSC_SET_IGNORE_CATCHING]
//          HitModule::set_invincible_frame_global(boma,
//              get_param_int(boma, "common", "throw_invincible_frame"), false, 0)
//
//   exit:  motion = hash40("invalid")
//          if next is THROW_F_F/_B/_HI/_LW  -> motion = that throw's own motion name
//          elif next is one of the eleven carry statuses -> nothing at all
//          else -> CatchModule::catch_cut(boma, false, false); set_ignore_catching(boma, false)
//          if motion != "invalid" -> WorkModule::set_int64(boma, motion,
//                                        FIGHTER_STATUS_CATCH_WAIT_WORK_INT_MOTION_KIND)
//                                    set_ignore_catching(boma, false)
//
// EVERY HASH IN THAT IS CONFIRMED, not inferred: 0x8af9b3d1d is hash40("shoulder"), 0x7fb997a80
// is hash40("invalid"), and 0x9c72950bb / 0x9c04494a2 / 0xa249b58ab / 0xabaf8a0cc are
// hash40("throw_f_f"/"throw_f_b"/"throw_f_hi"/"throw_f_lw") -- the same four names this module
// already passes to `throw_main`.
//
// WHAT EACH ONE BUYS.
//
// "shoulder" is how the victim is TOLD. `CaptureWait_Loop` in lua2cpp_common.nro reads a motion
// out of work and hands it to `CaptureModule::motion`; until something names the carry, they keep
// the pose the grab gave them, which is why the playtest saw "a spinning arms animation that looks
// like an animation that would happen when mario is falling".
//
// `set_ignore_catching(true)` stops the carry being interrupted by another catch.
//
// `throw_invincible_frame` off "common" is real invincibility DK has during the lift and this port
// did not -- and it is a COMMON param, so it reads correctly off Pikachu with no help from
// src/params.
//
// `catch_cut` on the way out is the release. Leaving a carry for WAIT without it leaves the
// engine's catch bookkeeping believing the grab is still live.
// ---------------------------------------------------------------------------------------------

/// The eleven statuses DK treats as "still carrying" -- ours are ten, since his four
/// carry-jump statuses are not reproduced (see ../status.rs).
fn still_carrying(status: i32) -> bool {
    status == RAT_STATUS_KIND_SHOULDER_START
        || status == RAT_STATUS_KIND_SHOULDER_WAIT
        || status == RAT_STATUS_KIND_SHOULDER_WALK
        || status == RAT_STATUS_KIND_SHOULDER_TURN
        || status == RAT_STATUS_KIND_SHOULDER_FALL
        || status == RAT_STATUS_KIND_SHOULDER_LANDING
}

/// The victim's thrown motion for each of the four cargo throws, exactly DK's mapping.
fn throw_motion(status: i32) -> Option<&'static str> {
    if status == RAT_STATUS_KIND_THROW_F_F {
        Some("throw_f_f")
    } else if status == RAT_STATUS_KIND_THROW_F_B {
        Some("throw_f_b")
    } else if status == RAT_STATUS_KIND_THROW_F_HI {
        Some("throw_f_hi")
    } else if status == RAT_STATUS_KIND_THROW_F_LW {
        Some("throw_f_lw")
    } else {
        None
    }
}

/// `ShoulderStart_init`, call for call.
/// DK's `SHOULDER_START` `INIT_STATUS`, donkey `0x71000083e0`, as an `Init` line rather than a call
/// from `Main`.
///
/// `INIT_STATUS` RUNS BEFORE `PRE`. Running this from `Main` put the carry's setup two status
/// functions later than the engine expects it -- `IGNORE_CATCHING` and the lift invincibility were
/// not in place while `pre` and the engine's own status entry ran. That ordering is not something
/// this port gets to choose.
unsafe extern "C" fn shoulder_start_init(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    enter_carry(boma, entry_of(boma));
    0.into()
}

unsafe fn enter_carry(boma: *mut BattleObjectModuleAccessor, entry: usize) {
    // ESTABLISH THE CAPTURE BEFORE SENDING THE EVENT, because the log said there was nothing to
    // send it to:
    //
    //     SHOULDER p0 carry start: NOTHING on LINK_NO_CAPTURE, so the native event is not sent
    //
    // Same failure the whip had, and the same fix -- see `whip/catch.rs`. The carry needs FOUR
    // things true before the victim's own status can do its job, and this had none of them:
    //
    //   1. the capture recorded, with us as master        `CaptureModule::capture`
    //   2. the link, on BOTH ends                          `LinkModule::link` twice
    //   3. the victim in the right status                  `CAPTURE_WAIT`, forced
    //   4. the captor's params answerable                  `src/params`, already true
    //
    // Then `L2CFighterCommon::status_CaptureWait` (common 0x71000b1150) takes over and does what
    // no port-side code should: `start_clatter_motion_rate`, the model constraint, and the escape
    // to `CAPTURE_CUT` via the `FIGHTER_STATUS_TRANSITION_TERM_ID_CAPTURE_CUT` term that
    // `CaptureWaitCommon` (0x71000b1980) tests every frame.
    //
    // `docs/the-recording.md` is the acceptance test for this: the real carry shows
    // `mario1 CaptureModule::motion(hash40("capture_pulled"))` then `capture_wait` every frame,
    // both logged against the VICTIM. Those are not calls to copy -- they are what the victim's
    // own status does once it is actually running.
    let them = VICTIM[entry];
    if let Some(vboma) = crate::victim::boma(them) {
        if !LinkModule::is_linked(boma, *LINK_NO_CAPTURE) {
            let me = (*boma).battle_object_id;
            CaptureModule::capture(vboma, me, *STATUS_KIND_NONE, true, 0);
            // Both ends: `Link(this, no, id)` writes only the CALLER's slot -- Ghidra on
            // main + 0x47f120. One call leaves the victim with no parent and every branch of
            // their status gated behind `is_link` does nothing.
            LinkModule::link(boma, *LINK_NO_CAPTURE, them);
            LinkModule::link(vboma, *LINK_NO_CAPTURE, me);
            // CAPTURE_WAIT, AND I BROKE THE ONE RULE THE RECORDING HAS BY CHANGING IT.
            //
            // I replaced this with FIGHTER_STATUS_KIND_SHOULDERED_DONKEY_START and it CRASHED the
            // game before the victim reached the shoulder: a Data Abort on MainThread at
            // main + 0x484da4, `ldr x8, [x25]` with x25 null, followed by `ldr x8, [x8, #0x3c8]`
            // and `blr x8` -- a virtual call through a null object inside the engine's path for
            // that status. Round 3's note called requesting those statuses "the leading suspect
            // for the freeze" and I overturned it on the strength of a status NAME and a read of
            // its script, which is generic. The script being generic does not make the engine's
            // path for it generic.
            //
            // AND THE ANSWER WAS ALREADY WRITTEN DOWN. `docs/the-recording.md` shows what the real
            // cargo does to its victim:
            //
            //     mario1  CaptureModule::motion(hash40("capture_pulled"), 0)
            //     mario1  CaptureModule::motion(hash40("capture_wait"), 0)   every frame carried
            //
            // `capture_wait` is CAPTURE_WAIT's motion. The victim of a real cargo carry is in
            // CAPTURE_WAIT, which is also why this module's own comment describes the cargo as "an
            // ordinary capture that was re-caught rather than released". That page states its own
            // precedence -- "where this page and any reasoning elsewhere in the repo disagree,
            // this page wins" -- and I disagreed with it from a symbol name.
            //
            // WHAT THE SHOULDERED_DONKEY FAMILY IS FOR IS STILL UNKNOWN, and the interesting part
            // is that reading it told me nothing useful. `status_ShoulderedDonkeyStart`
            // (0x710025b0b0) reads the VICTIM's own param_motion and changes motion;
            // `status_ShoulderedDonkey` (0x710025bf50) plays `shouldered_donkey`, starts the
            // clatter, sets a constraint offset and calls `CameraModule::set_being_carried`;
            // `exec_fix_pos_counter` (0x710025b680) decays the mash timer off the CARRIER's
            // situation kind. Nothing in any of it mentions Donkey Kong, every id it touches is a
            // `FIGHTER_STATUS_` common one, and 0xC5/0xC6 are common status kinds. By every static
            // measure it looked safe, and it crashed on the first frame.
            //
            // That is the lesson worth keeping: a status's SCRIPT being generic says nothing about
            // the engine's path into it. Left alone until something measures it rather than reads
            // it.
            crate::victim::request_status(
                vboma,
                *FIGHTER_STATUS_KIND_CAPTURE_WAIT,
                "cargo carry start",
            );
            crate::telemetry::sink::diag(format!(
                "[rat] SHOULDER p{entry} established capture on {them:#x}: is_capture={} \
                 their_is_link={} their_st={:#x}",
                CaptureModule::is_capture(vboma),
                LinkModule::is_link(vboma, *LINK_NO_CAPTURE),
                StatusModule::status_kind(vboma)
            ));
        }
    }

    if linked_victim(boma, entry, "carry start") {
        LinkModule::send_event_nodes(boma, *LINK_NO_CAPTURE, Hash40::new("shoulder"), 0);
    }
    CaptureModule::set_ignore_catching(boma, true);
    // "common" is every fighter's own param object, so this is Pikachu's number and not
    // Donkey Kong's -- which is correct: it is how long HE is invincible for while lifting.
    let frames = WorkModule::get_param_int(boma, hash40("common"), hash40("throw_invincible_frame"));
    HitModule::set_invincible_frame_global(boma, frames, false, 0);
    crate::telemetry::sink::diag(format!(
        "[rat] SHOULDER p{entry} carry protocol: sent \"shoulder\" to LINK_NO_CAPTURE, \
         ignore_catching on, {frames}f invincible"
    ));
}

/// `ShoulderStart_exit`, call for call, for a transition to `next`.
unsafe fn leave_carry(boma: *mut BattleObjectModuleAccessor, entry: usize, next: i32) {
    if let Some(motion) = throw_motion(next) {
        WorkModule::set_int64(
            boma,
            Hash40::new(motion).hash as u64,
            *FIGHTER_STATUS_CATCH_WAIT_WORK_INT_MOTION_KIND,
        );
        CaptureModule::set_ignore_catching(boma, false);
        crate::telemetry::sink::diag(format!(
            "[rat] SHOULDER p{entry} carry protocol: throwing, victim motion {motion}"
        ));
        return;
    }
    if still_carrying(next) {
        return;
    }
    CatchModule::catch_cut(boma, false, false);
    CaptureModule::set_ignore_catching(boma, false);
    crate::telemetry::sink::diag(format!(
        "[rat] SHOULDER p{entry} carry protocol: released -- catch_cut, ignore_catching off"
    ));
}

unsafe fn carry_change(fighter: &mut L2CFighterCommon, entry: usize, next: i32, why: &str) -> L2CValue {
    if !crate::victim::allow_shift(entry, why) {
        return 0.into();
    }
    // `leave_carry` USED TO BE CALLED HERE, and the reasoning for that was wrong in one specific
    // way. The old comment said: "DK does this in the status EXIT line, which smashline supports --
    // but every one of these transitions already funnels through here, and one place that knows
    // both the old and the new status is easier to keep honest than ten registered exits."
    //
    // "Every one of these transitions" is only true of the transitions THE PORT DRIVES. Getting
    // hit, being grabbed, dying, falling off the stage -- the engine changes status without ever
    // passing through `carry_change`, and the teardown then never ran at all: the victim stayed
    // attached and `CATCH_WAIT_WORK_INT_MOTION_KIND` was never written. DK's `EXIT_STATUS` at
    // `0x7100004df0` runs on EVERY exit, which is the entire reason it is an exit line and not a
    // call at the top of each transition.
    //
    // It is now registered as `Exit` on all six statuses (see `install`), so it covers this
    // transition too -- calling it here as well would run the teardown twice.
    fighter.change_status(next.into(), false.into());
    1.into()
}

/// DK's shared `SHOULDER_*` `EXIT_STATUS`, gaogaen -- no, donkey `0x7100004df0`, registered on all
/// eleven of his carry statuses and now on all six of ours.
///
/// IT DECIDES BY THE STATUS BEING ENTERED, read from `global_table[0xb]`:
///
///   * one of the four `THROW_F_*` -> write that throw's motion into
///     `FIGHTER_STATUS_CATCH_WAIT_WORK_INT_MOTION_KIND`, which is the work id the port's own
///     `pikachu_throw_main` already reads back
///   * another `SHOULDER_*` -> do nothing; the carry continues and there is nothing to tear down
///   * anything else -> the carry is being lost, so write `invalid`
///
/// `leave_carry` already reproduced this body call for call. All that was wrong was where it ran.
unsafe extern "C" fn shoulder_exit(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    // `global_table[0xb]` is the incoming status. `specialhi` already reads it this way.
    let next = fighter.global_table[0xb].get_i32();
    if is_carry_status(next) {
        // Still carrying: DK's helper jumps straight past the teardown for this case.
        return 0.into();
    }
    leave_carry(boma, entry, next);
    0.into()
}

/// The six statuses that mean "still carrying", so `shoulder_exit` can tell a carry-to-carry
/// transition from losing the carry. DK tests all eleven of his; these are the six this port has.
fn is_carry_status(kind: i32) -> bool {
    kind == RAT_STATUS_KIND_SHOULDER_START
        || kind == RAT_STATUS_KIND_SHOULDER_WAIT
        || kind == RAT_STATUS_KIND_SHOULDER_WALK
        || kind == RAT_STATUS_KIND_SHOULDER_TURN
        || kind == RAT_STATUS_KIND_SHOULDER_FALL
        || kind == RAT_STATUS_KIND_SHOULDER_LANDING
}

/// Is there actually a fighter on the capture link, and says so once per carry.
///
/// THIS GUARD IS A DEVIATION FROM THE SOURCE, ON PURPOSE, AND IT COST A CRASH. Donkey Kong's
/// `ShoulderStart_init` and `ThrowF*_init` send their events with no check at all, because for him
/// the link is guaranteed: he only ever reaches those statuses out of a catch that connected. This
/// port reaches them out of a hand-detected grab, and the measurement says the link is not there:
///
///   [rat] VICTIM p0 cargo: status=0x44 captured=false master=0x50000000 clatter=65.0
///
/// `captured=false`, `master=BATTLE_OBJECT_ID_INVALID`, and status 0x44 is CAPTURE_CUT.
///
/// `LinkModule::send_event_nodes_throw` with nothing on the link is what crashed the throw -- a
/// Data Abort at fault address 0 on MainThread, no plugin frame in the backtrace, faulting at
/// `cross2 + 0x3e614c`:
///
///     ldrh w9,  [x28, #0x28]
///     ldr  x10, [x19, #0x18]      <- the array base, null
///     madd x9, x9, x22, x10       <- x22 = 0x2d0, the stride
///     ldrb w8,  [x28, #0x2a]
///     add  x8, x9, x8, lsl #3
///     ldr  x0, [x8, #0x30]        <- the fault
///
/// an engine table indexed off a null base. The event is delivered by walking the link's nodes,
/// and there were none.
///
/// ADR 0004 is exactly this rule: engine code may not be handed something it assumes the source
/// fighter has. The precondition Donkey Kong gets for free has to be checked here.
unsafe fn linked_victim(boma: *mut BattleObjectModuleAccessor, entry: usize, who: &str) -> bool {
    let linked = LinkModule::is_linked(boma, *LINK_NO_CAPTURE);
    if !linked {
        crate::telemetry::sink::crumb(format!(
            "[rat] SHOULDER p{entry} {who}: NOTHING on LINK_NO_CAPTURE, so the native event is \
             not sent -- the engine is not holding this victim for us"
        ));
    }
    linked
}
