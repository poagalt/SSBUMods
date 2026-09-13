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
/// `common/turn_frame`, the countdown ShoulderTurn_main writes into
/// FIGHTER_STATUS_TURN_WORK_FLOAT_TURN_FRAME so the engine knows when to flip him.
const TURN_FRAME: &str = "turn_frame";

/// The carried fighter's battle object id, per entry.
///
/// THE ID FORMAT IS MEASURED, NOT GUESSED. Three features were blocked on turning an entry id
/// into a battle object id, and rather than guess, the telemetry logged what the engine itself
/// hands back during Pikachu's ordinary grab. The answer: `captured object id = 0x10000` while we
/// were entry 0 and the victim was entry 1. So a fighter's object id is `entry << 16`, not the
/// `0x5000000n` the invalid sentinel suggested. Kept here as the id directly, since that is what
/// `sv_battle_object` wants.
// `crate::victim::NOBODY`, not `0`: player one's battle object id IS zero, so a zero here
// resolves to a real fighter. See the note on `crate::victim::HELD_ID`.
static mut VICTIM: [u32; 8] = [crate::victim::NOBODY; 8];
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

    // NO HAND-WRITTEN FALLBACK ANY MORE. If the engine is not holding them, THE CARRY IS OVER.
    //
    // This used to read Pikachu's `shoulderl` joint and write the victim's posture, with a further
    // fallback to the fighter's own position plus 8 when the joint returned the origin. Both are
    // now removed, and the reason is that the fallback was the bug rather than the safety net:
    // once the victim mashed out, the port kept positioning a fighter who had escaped, which is
    // the reported "after some time mario still gets stuck to pikachu's feet".
    //
    // The fallback was written when the port never established a capture, so the engine genuinely
    // was not placing anybody and hand-placement was all there was. That changed: `enter_carry`
    // now records the capture and links both ends, and the playtest log confirms the engine takes
    // over -- "leaving their position to CaptureModule" on every frame of a working carry.
    //
    // So there are exactly two states, and neither of them wants this code. Held: the engine
    // places them, the early return above fires. Not held: `still_held` has gone false and the
    // carry ends. Writing a captured fighter's transform from two places was also the leading
    // suspect for the cargo Instruction Abort, and this removes the only cross-object write in the
    // whole carry.
    let _ = vboma;
    crate::telemetry::sink::crumb(format!(
        "[rat] SHOULDER p{entry} victim {:#x} is NOT in our capture -- not positioning them; \
         still_held will end the carry",
        VICTIM[entry]
    ));
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
    if crate::victim::held(entry).is_none() {
        return false;
    }
    // THE CLATTER RUNNING OUT IS THE ENGINE'S OWN END-OF-CARRY, AND MISSING IT IS WHY THE VICTIM
    // ENDED UP AT PIKACHU'S FEET AFTER A WHILE.
    //
    // `src/victim.rs` has quoted this condition since round four without anything acting on it:
    //
    //     CaptureWaitCommon:  if ControlModule::get_clatter_time(boma) <= 0.0 -> escape
    //
    // and the last playtest shows it happening, twice, on the carried fighter:
    //
    //     VICTIM p0 cargo: status=0xc6 captured=true clatter=102.0
    //     VICTIM p0 cargo: status=0xc6 captured=true clatter=0.0      <- mashed out
    //     VICTIM p0 cargo: status=0xc5 captured=true clatter=86.0     <- and re-entered
    //
    // Clatter reaching zero is Mario escaping, which is correct and is the whole point of mashing.
    // The port did not notice, kept believing it held him, and `hold_victim` fell through to
    // hand-writing his posture -- which is the reported "after some time mario still gets stuck to
    // pikachu's feet". The 170-frame ceiling then fired as a backstop and logged
    // "hold ceiling reached -- something never let go", which was the symptom, not the cause.
    //
    // The comment above says the engine's link "absence proves nothing", and that WAS true when
    // the port never established a capture. It does now -- the same log line says
    // `captured=true` -- so the engine's answer is load-bearing again and this is the question it
    // answers.
    if let Some(vboma) = crate::victim::boma(VICTIM[entry]) {
        if ControlModule::get_clatter_time(vboma, 0) <= 0.0 {
            crate::telemetry::sink::diag(format!(
                "[rat] SHOULDER p{entry} victim {:#x} mashed out (clatter 0) -- ending the carry, \
                 which is what CaptureWaitCommon does",
                VICTIM[entry]
            ));
            // RELEASED HERE, NOT LEFT TO THE CEILING. Returning false ends the carry and
            // `shoulder_exit` cuts the catch, but nothing dropped the hold -- so `HELD_ID` stayed
            // set and `tick_hold`'s ceiling was what finally let go, seconds later. That is the
            // reported "t-posing stuck behavior... becomes dislodged after a few seconds": the
            // few seconds are the ceiling. `release` requests CAPTURE_CUT on them, which is how
            // the engine pops an escapee out of an ordinary grab.
            crate::victim::release(entry, "victim mashed out of the carry");
            return false;
        }
    }
    true
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
/// EVERY ARGUMENT IS NOW PER-STATUS, BECAUSE DK'S ARE.
///
/// This helper used to hardcode four of them and it was wrong on three counts, each measurable:
///
///   * `SITUATION_KIND_NONE` for all ten. DK names the real situation in every one --
///     GROUND for Start/Wait/Walk/Turn/Landing/JumpSquat, AIR for Fall/Jump/Pass. `init_settings`
///     uses it to decide whether the status is entered as a grounded or airborne one, which is
///     what tells the engine's ground correction and the fall machinery what they are looking at.
///   * `NONE_FLAG/_INT/_FLOAT` for all ten. ShoulderFall passes the three `FALL` keep flags,
///     because it is a fall and wants the fall work preserved.
///   * `set_fighter_status_data`'s 7th argument always 0. ShoulderWalk passes
///     `FIGHTER_STATUS_ATTR_SCALE_KINETIC_ENERGY` -- and ShoulderWalk is the one status in the
///     carry whose kinetic type is MOTION_LOOP, i.e. the one where the animation's own
///     displacement is the movement. That attribute is how the engine scales it. Without it the
///     walk had nothing to scale and nothing to move, which is half of "cargo throw can only move
///     the opposite direction that the initial grab happened in".
///
/// Argument names follow skyline-smash's own (`arg2`, `grab_invun`, `arg6`, `arg8`), because
/// guessing at better ones would be inventing meaning the binding does not have.
#[allow(clippy::too_many_arguments)]
unsafe fn shoulder_pre(
    fighter: &mut L2CFighterCommon,
    situation: i32,
    kinetic: i32,
    ground_correct: i32,
    keep: (i32, i32, i32),
    arg2: bool,
    arg6: bool,
    arg8: u32,
) -> L2CValue {
    StatusModule::init_settings(
        fighter.module_accessor,
        smash::app::SituationKind(situation),
        kinetic,
        ground_correct as u32,
        smash::app::GroundCliffCheckKind(*GROUND_CLIFF_CHECK_KIND_NONE),
        true,
        keep.0,
        keep.1,
        keep.2,
        *FS_SUCCEEDS_KEEP_ATTACK_ABSOLUTE,
    );
    FighterStatusModuleImpl::set_fighter_status_data(
        fighter.module_accessor,
        arg2,
        *FIGHTER_TREADED_KIND_NO_REAC,
        false,
        false,
        arg6,
        0,
        arg8,
        0,
        0,
    );
    0.into()
}

/// `FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_*`, the triple eight of the ten carry statuses pass.
unsafe fn keep_none() -> (i32, i32, i32) {
    (
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLAG,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_INT,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLOAT,
    )
}

// ---- SHOULDER_START : the lift, out of throw_f ----------------------------------------------

unsafe extern "C" fn shoulder_start_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(fighter, *SITUATION_KIND_GROUND, *FIGHTER_KINETIC_TYPE_MOTION, *GROUND_CORRECT_KIND_GROUND_OTTOTTO, keep_none(), true, false, 0)
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

/// OFF, BECAUSE THE ENGINE ALREADY DOES IT AND ASKING TWICE CRASHED.
///
/// The playtest that carried Mario twice and died on the third attempt ends on this function's own
/// line, and the line answers the whole question:
///
///     SHOULDER p0 shouldering 0x10000 on carry frame 2 (their st=0xc5)
///
/// `their st` is read BEFORE the request, and 0xc5 IS
/// `FIGHTER_STATUS_KIND_SHOULDERED_DONKEY_START`. The victim was already in it. The engine moves
/// them there itself in response to the `hash40("shoulder")` event `enter_carry` sends to
/// LINK_NO_CAPTURE -- which is exactly the native path, and exactly what `docs/the-recording.md`
/// shows Donkey Kong doing with that event.
///
/// So the port's job was only ever to send the event, and it already did. Forcing a status the
/// fighter is already in re-runs its entry, and the engine's path for a re-entry is what faulted.
/// The two carries that worked are the evidence that the native path is sufficient on its own.
///
/// Kept rather than deleted because the reasoning above is only legible next to the code it
/// rejects, and because the earlier crash analysis (the link slot's `+0x58` cached pointer at
/// main + 0x484da4) is still the right reading of THAT crash -- the frame-2 delay and the node
/// precondition are what let the first two carries succeed at all.
const SHOULDER_THE_VICTIM: bool = false;

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
    shoulder_pre(fighter, *SITUATION_KIND_GROUND, *FIGHTER_KINETIC_TYPE_MOTION, *GROUND_CORRECT_KIND_GROUND_OTTOTTO, keep_none(), true, false, 0)
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

    // THE FIRST THING DK'S HELPER DOES, AND THIS PORT ONLY DESCRIBED IT. The comment above has
    // listed `ControlModule::get_clatter_time` on the victim, "`<= 0` returns early with no
    // throw", since the helper was first read -- and nothing implemented it. Straight from
    // donkey 0x7100022520, the first eight calls of the function:
    //
    //     LinkModule::get_node_object_id(boma, LINK_NO_CAPTURE)
    //     sv_battle_object::module_accessor(that)
    //     ControlModule::get_clatter_time(that, 0)
    //     operator<=(0.0)      -> return false
    //
    // A victim whose mash timer has hit zero is a victim who is leaving, and throwing them on the
    // frame they escape is a throw with nobody in it. The node id is read off the LINK rather
    // than off our own bookkeeping, which is DK's source of truth too.
    let them = LinkModule::get_node_object_id(boma, *LINK_NO_CAPTURE) as u32;
    if let Some(vboma) = crate::victim::boma(them) {
        if ControlModule::get_clatter_time(vboma, 0) <= 0.0 {
            return None;
        }
    }

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
    // `IsThrowStick()` RETURNS A TABLE, NOT A BOOL, AND THAT IS THE WHOLE OF "CARGO DOWN AND UP
    // THROW RESULT IN FORWARD THROW WITH THE LEFT STICK BUT WORK FINE WITH THE C-STICK".
    //
    // DK's helper, disassembled to the branch (donkey 0x7100022744 onward):
    //
    //     btn = globalTable[0x20] & (CAT1_SPECIAL_N | _SPECIAL_S | _SPECIAL_HI | _SPECIAL_LW
    //                                | _ATTACK_N)
    //     stick = self:IsThrowStick()
    //     if btn and (globalTable[0x21] & CAT2_THROW_F  or stick["f"])  -> THROW_F_F,  true
    //     if btn and (globalTable[0x21] & CAT2_THROW_B  or stick["b"])  -> THROW_F_B,  true
    //     if btn and (globalTable[0x21] & CAT2_THROW_HI or stick["hi"]) -> THROW_F_HI, true
    //     if btn and (globalTable[0x21] & CAT2_THROW_LW or stick["lw"]) -> THROW_F_LW, true
    //
    // The four `operator[](phx::Hash40)` calls resolve to `f`, `b`, `hi` and `lw` -- four fields of
    // a table the engine fills in from its own throw-stick logic. This port read the return value
    // as `IsThrowStick().get_bool()` and then invented +/-0.5 stick thresholds beside it, then
    // deleted those and defaulted to forward, which is why the C-stick still worked and the left
    // stick did not: the C-stick raises a cat2 THROW command flag, and the left stick with an
    // attack press raises none -- its direction is ONLY in that table.
    //
    // THE BUTTON IS THE CAT1 COMMAND MASK, which is also DK's and replaces a
    // `check_button_trigger` pair. The note that pair replaced is still true -- a cat2 flag alone
    // as the trigger threw the victim on the first carry frame and froze the game in round 2 --
    // but cat1 is not cat2: those five are ATTACK/SPECIAL *commands*, recognised once per input by
    // the input system, and they are what DK gates on. `MAX_SHIFTS_PER_FRAME` did not exist when
    // that freeze happened and now caps the whole class at eight changes a frame.
    let btn = fighter.global_table[0x20].get_i32()
        & (*FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_N
            | *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_S
            | *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_HI
            | *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_LW
            | *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_N);
    if btn == 0 {
        return None;
    }
    let cat2 = fighter.global_table[0x21].get_i32();
    let stick = fighter.IsThrowStick();
    // DK tests them in this order: f, b, hi, lw.
    for (mask, name, status) in [
        (*FIGHTER_PAD_CMD_CAT2_FLAG_THROW_F, "f", RAT_STATUS_KIND_THROW_F_F),
        (*FIGHTER_PAD_CMD_CAT2_FLAG_THROW_B, "b", RAT_STATUS_KIND_THROW_F_B),
        (*FIGHTER_PAD_CMD_CAT2_FLAG_THROW_HI, "hi", RAT_STATUS_KIND_THROW_F_HI),
        (*FIGHTER_PAD_CMD_CAT2_FLAG_THROW_LW, "lw", RAT_STATUS_KIND_THROW_F_LW),
    ] {
        if cat2 & mask != 0 || stick[name].try_get_bool().unwrap_or(false) {
            return Some(status);
        }
    }
    // NO DIRECTION AT ALL IS NOT A THROW FOR DK -- he has four tests and no `else`, so an
    // attack press with a neutral stick falls out of the helper and the carry continues.
    //
    // This used to return forward here, on the reasoning that "a cargo attack press with the
    // stick neutral is a forward throw in the real move". That was an invention twice over: it is
    // not what the source does, and the four `stick[...]` fields are exactly the engine's own
    // answer to "was a direction asked for". Logged rather than silently swallowed, because a
    // button press that produces no direction at all is the input this port has most often got
    // wrong.
    crate::telemetry::sink::crumb(format!(
        "[rat] SHOULDER throw: cat1 press with no direction (cat2={cat2:#x}, stick f/b/hi/lw = \
         {}/{}/{}/{}) -- not a throw, per DK's four tests and no else",
        stick["f"].try_get_bool().unwrap_or(false),
        stick["b"].try_get_bool().unwrap_or(false),
        stick["hi"].try_get_bool().unwrap_or(false),
        stick["lw"].try_get_bool().unwrap_or(false),
    ));
    None
}

/// `common/pass_stick_y`, `common/pass_flick_y` -- the soft-platform drop, and the FIRST thing
/// both of DK's ground loops test.
const PASS_STICK_Y: &str = "pass_stick_y";
const PASS_FLICK_Y: &str = "pass_flick_y";
/// `common/jump_stick_y`, `common/jump_flick_y`, `common/jump_neutral_y` -- the jump out of the
/// carry.
const JUMP_STICK_Y: &str = "jump_stick_y";
const JUMP_FLICK_Y: &str = "jump_flick_y";
const JUMP_NEUTRAL_Y: &str = "jump_neutral_y";

/// `global_table[0x1a]`, THE ENGINE'S OWN CACHED STICK X, TIMES `lr`.
///
/// THIS IS THE WHOLE ANSWER TO "CARGO THROW CAN ONLY MOVE THE OPPOSITE DIRECTION THAT THE INITIAL
/// GRAB HAPPENED IN".
///
/// Both of DK's ground loops compute exactly this expression and compare it against a param. The
/// port compared `ControlModule::get_stick_x(boma).abs()` against one threshold and
/// `stick_x * lr` against the negation of another, which is three separate departures:
///
///   * `abs()` throws away the sign, so "stick forward" and "stick backward" were the same input
///     and the walk could only ever go one way -- the reported symptom, exactly.
///   * the thresholds were read with `get_param_float(boma, hash40("turn_stick_x"), 0)`, passing
///     the KEY as the param OBJECT and 0 as the key. The engine wants
///     `(hash40("common"), hash40("turn_stick_x"))`. Both reads returned 0, and `.max(0.1)` then
///     silently substituted a made-up 0.1 for both -- see `docs/decisions.md` on never inventing
///     a number.
///   * the turn test negated the param. DK does not: `common/turn_stick_x` is already the
///     negative threshold, so negating it flips the whole comparison.
///
/// `0x1a` is the cached stick x rather than a fresh `ControlModule` read, and that is checked, not
/// assumed: the same `globalTable[0x1a] * lr` appears in Game & Watch's `FinalRun_main_loop`
/// compared against `common/turn_run_stick_x`, and in DK's ShoulderWait compared against
/// `common/turn_stick_x` and `common/walk_stick_x` -- three stick-x thresholds, one slot. The
/// multiply is `_ZNK3lib8L2CValuemlERKS0_`, resolved out of the NRO's own .dynsym rather than read
/// off Ghidra's collapsed `operator_` name.
unsafe fn stick_x_lr(fighter: &mut L2CFighterCommon) -> f32 {
    fighter.global_table[0x1a].get_f32() * PostureModule::lr(fighter.module_accessor)
}

unsafe fn common_param_f(boma: *mut BattleObjectModuleAccessor, key: &str) -> f32 {
    WorkModule::get_param_float(boma, hash40("common"), hash40(key))
}

unsafe fn common_param_i(boma: *mut BattleObjectModuleAccessor, key: &str) -> i32 {
    WorkModule::get_param_int(boma, hash40("common"), hash40(key))
}

/// DK's soft-platform drop test, shared by ShoulderWait (donkey 0x7100024240) and ShoulderWalk
/// (0x7100023cd0), where it is the first thing either of them checks after the throw and jump
/// helpers:
///
///     if get_stick_y() <= get_param_float(common, pass_stick_y)
///        and globalTable[0x1d] < get_param_int(common, pass_flick_y)
///        and globalTable[0x16] == SITUATION_KIND_GROUND
///        and GroundModule::is_passable_check()
///     then change_status(SHOULDER_PASS, true) end
///
/// `globalTable[0x1d]` is the flick-y age in frames -- it is compared with an INT param called
/// `pass_flick_y`, and the same slot appears in the jump helper against `jump_flick_y`.
unsafe fn shoulder_pass_check(fighter: &mut L2CFighterCommon, entry: usize) -> bool {
    let boma = fighter.module_accessor;
    if ControlModule::get_stick_y(boma) <= common_param_f(boma, PASS_STICK_Y)
        && fighter.global_table[0x1d].get_i32() < common_param_i(boma, PASS_FLICK_Y)
        && fighter.global_table[0x16] == *SITUATION_KIND_GROUND
        && GroundModule::is_passable_check(boma)
    {
        carry_change_input(fighter, entry, RAT_STATUS_KIND_SHOULDER_PASS, "SHOULDER_PASS");
        return true;
    }
    false
}

/// DK's JUMP-OUT-OF-THE-CARRY, `FUN_7100023820`, transcribed. Called every frame by ShoulderWait,
/// ShoulderWalk and ShoulderTurn -- three of his eleven -- and by nothing else.
///
/// THIS FUNCTION IS "I ALSO CAN'T JUMP WHILE IN CARGO THROW". It did not exist here.
///
///     if (check_button_trigger(CONTROL_PAD_BUTTON_JUMP)
///         or (globalTable[0x20] & FIGHTER_PAD_CMD_CAT1_FLAG_JUMP_BUTTON))
///        and globalTable[0x16] == SITUATION_KIND_GROUND then
///         change_status(SHOULDER_JUMP_SQUAT_B, true); return true
///     end
///     if get_param_float(common, jump_stick_y) <= get_stick_y()
///        and globalTable[0x1d] < get_param_int(common, jump_flick_y)
///        and globalTable[0x16] == SITUATION_KIND_GROUND
///        and ControlModule::is_enable_flick_jump() then
///         change_status(SHOULDER_JUMP_SQUAT, true); return true
///     end
///     return false
///
/// TWO JUMPSQUATS, AND THE SUFFIX IS THE INPUT, NOT THE DIRECTION. `_B` is the one the BUTTON
/// reaches and the plain one is the one the STICK FLICK reaches -- which reads backwards until you
/// see what the difference is for: `FUN_7100022f30` tests `status_kind == SHOULDER_JUMP_SQUAT_B`
/// and, if so, watches for the jump button being RELEASED during the squat to reserve a short
/// hop. A flick jump has no button to release, so it decides the same thing from
/// `common/jump_neutral_y` instead. Two statuses because the short-hop rule differs, not because
/// the jump does.
unsafe fn shoulder_jump_check(fighter: &mut L2CFighterCommon, entry: usize) -> bool {
    let boma = fighter.module_accessor;
    let pressed = ControlModule::check_button_trigger(boma, *CONTROL_PAD_BUTTON_JUMP)
        || (fighter.global_table[0x20].get_i32() & *FIGHTER_PAD_CMD_CAT1_FLAG_JUMP_BUTTON) != 0;
    if pressed && fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
        crate::telemetry::probe::event(format!(
            "[rat] SHOULDER p{entry} jump (button) out of the carry after {}f",
            CARRY_FRAMES[entry]
        ));
        carry_change_input(
            fighter,
            entry,
            RAT_STATUS_KIND_SHOULDER_JUMP_SQUAT_B,
            "SHOULDER_JUMP_SQUAT_B",
        );
        return true;
    }
    if common_param_f(boma, JUMP_STICK_Y) <= ControlModule::get_stick_y(boma)
        && fighter.global_table[0x1d].get_i32() < common_param_i(boma, JUMP_FLICK_Y)
        && fighter.global_table[0x16] == *SITUATION_KIND_GROUND
        && ControlModule::is_enable_flick_jump(boma)
    {
        crate::telemetry::probe::event(format!(
            "[rat] SHOULDER p{entry} jump (flick) out of the carry after {}f",
            CARRY_FRAMES[entry]
        ));
        carry_change_input(
            fighter,
            entry,
            RAT_STATUS_KIND_SHOULDER_JUMP_SQUAT,
            "SHOULDER_JUMP_SQUAT",
        );
        return true;
    }
    false
}

/// The two things every grounded carry loop does before it looks at the stick: DK's
/// `FUN_7100022520` (the throw) and `FUN_7100023820` (the jump), in that order, and either one
/// returning true ends the frame.
///
/// Returns `Some(())` when one of them took the frame.
unsafe fn carry_interrupts(fighter: &mut L2CFighterCommon, entry: usize) -> bool {
    let boma = fighter.module_accessor;
    if let Some(next) = requested_throw(fighter) {
        crate::telemetry::probe::event(format!(
            "[rat] SHOULDER p{entry} throw {} after {}f; {}",
            crate::telemetry::probe::st(next),
            CARRY_FRAMES[entry],
            crate::telemetry::probe::ctx(boma)
        ));
        carry_change_input(fighter, entry, next, "requested throw");
        return true;
    }
    shoulder_jump_check(fighter, entry)
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
    // DK's two helpers, in his order: the throw, then the jump.
    if carry_interrupts(fighter, entry) {
        return 1.into();
    }
    // Then the soft-platform drop, which is his third test and not his last.
    if shoulder_pass_check(fighter, entry) {
        return 1.into();
    }

    // DONKEY KONG'S SHOULDERWAIT DECISION, donkey 0x7100024240, transcribed:
    //
    //     if globalTable[0x1a] * lr <= get_param_float(common, turn_stick_x)
    //        and globalTable[0x16] == SITUATION_KIND_GROUND      -> SHOULDER_TURN, true
    //     if get_param_float(common, walk_stick_x) <= globalTable[0x1a] * lr
    //        and globalTable[0x16] == SITUATION_KIND_GROUND      -> SHOULDER_WALK, true
    //     if globalTable[0x16] == SITUATION_KIND_AIR             -> SHOULDER_FALL, false
    //     if MotionModule::is_end() and motion_kind() != hash40("shoulder_wait")
    //                                                            -> change_motion(shoulder_wait)
    //
    // NOTE THE ARGUMENT ORDER OF THE TWO COMPARISONS. Turn is `stick <= turn_stick_x`; walk is
    // `walk_stick_x <= stick`. They are the same operator (`_ZNK3lib8L2CValueleERKS0_`) with the
    // operands swapped, which is how one slot and two params give a signed window: below the
    // negative threshold is a turn, above the positive one is a walk, between them is neither.
    // `.abs()` cannot express that, which is why the port could only walk one way.
    //
    // THE AIR TEST MOVED. It used to be first, and `!= GROUND` rather than `== AIR` -- so any
    // situation that is neither (CLIFF, for one) fell into SHOULDER_FALL. DK tests `== AIR` and
    // tests it last, after the stick.
    let stick = stick_x_lr(fighter);
    let ground = fighter.global_table[0x16] == *SITUATION_KIND_GROUND;
    if stick <= common_param_f(boma, TURN_STICK_X) && ground {
        return carry_change_input(fighter, entry, RAT_STATUS_KIND_SHOULDER_TURN, "SHOULDER_TURN");
    }
    if common_param_f(boma, WALK_STICK_X) <= stick && ground {
        return carry_change_input(fighter, entry, RAT_STATUS_KIND_SHOULDER_WALK, "SHOULDER_WALK");
    }
    if fighter.global_table[0x16] == *SITUATION_KIND_AIR {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_FALL, "SHOULDER_FALL");
    }
    // HIS ANIMATION REFRESH, WHICH IS WHY THE LOG SAID `WAIT entered` FOUR TIMES A CARRY.
    //
    // `shoulder_wait_main` changes motion on entry, so the port's only way to loop the pose was
    // to re-enter the status -- and re-entering WAIT is what the measured log shows happening,
    // four times in one carry with nothing else between. DK refreshes it in place instead, and
    // only when the motion has actually ended and is not already the one he wants.
    if MotionModule::is_end(boma) && MotionModule::motion_kind(boma) != hash40("shoulder_wait") {
        MotionModule::change_motion(
            boma,
            Hash40::new("shoulder_wait"),
            0.0,
            1.0,
            false,
            0.0,
            false,
            false,
        );
    }
    0.into()
}

// ---- SHOULDER_WALK ---------------------------------------------------------------------------

unsafe extern "C" fn shoulder_walk_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    // MOTION_LOOP: the animation moves him. This is the fix for the unbounded speed.
    shoulder_pre(
        fighter,
        *SITUATION_KIND_GROUND,
        *FIGHTER_KINETIC_TYPE_MOTION_LOOP,
        *GROUND_CORRECT_KIND_GROUND_OTTOTTO,
        keep_none(),
        true,
        false,
        // THE ONE ARGUMENT THAT IS NOT ZERO IN ANY OF THE TEN. See `shoulder_pre`.
        *FIGHTER_STATUS_ATTR_SCALE_KINETIC_ENERGY as u32,
    )
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
/// THE COMMENT THAT USED TO STAND HERE WAS THE LOAD-BEARING WRONG FACT IN THIS WHOLE MODULE. It
/// said: "His `EXEC_STATUS` for this status (`0x7100006170`) contains no calls at all -- the
/// per-frame drive is entirely the common walk code, which is the point of routing through it."
///
/// `0x7100006170` is `L2CFighterDonkey::status::ShoulderWalk_exec` and it contains ONE call, which
/// is the per-frame drive:
///
///     sub_walk_uniq_process_main_common("shoulder_walk_fast", "shoulder_walk_middle",
///                                       "shoulder_walk_slow", lib::L2CValue::NIL)
///
/// So "the per-frame drive is entirely the common walk code" was right and "contains no calls at
/// all" was wrong about the same three lines of assembly, and because the port believed the second
/// half it registered no `Exec` -- which means `calc_walk_speed` and `set_walk_motion` ran once,
/// at `INIT_STATUS`, and never again. A three-speed walk whose speed is chosen once is a walk
/// that does not move.
///
/// HOW THE OLD READING HAPPENED, since the same mistake is cheap to repeat: I read the
/// disassembly of the wrong address and did not cross-check it against
/// `vendor/ssbu-status/.../ShoulderWalk/exec.c`, which has had the call in it the whole time. The
/// named decomp is the cheaper source and it should be read first; the disassembler is for what
/// the decomp could not name.
///
/// THE ARGUMENT TYPES ARE ALSO FIXED HERE. These were `L2CValue::new_int(hash)`. DK constructs
/// them with `_ZN3lib8L2CValueC1EN3phx6Hash40E` -- the Hash40 constructor, PLT stub 0x7100120a20,
/// the same stub for all three -- so they are L2C values of type Hash, not Int, and the fourth
/// argument is `_ZN3lib8L2CValueC1Eb` with `wzr`, so a Bool false rather than an Int 0. L2C is
/// typed; `as_hash()` on an Int is not the same read.
unsafe extern "C" fn shoulder_walk_init(fighter: &mut L2CFighterCommon) -> L2CValue {
    fighter.sub_walk_uniq_process_init_common(
        L2CValue::new_hash(Hash40::new("shoulder_walk_fast").hash),
        L2CValue::new_hash(Hash40::new("shoulder_walk_middle").hash),
        L2CValue::new_hash(Hash40::new("shoulder_walk_slow").hash),
        L2CValue::new_bool(false),
    );
    0.into()
}

/// DK's `ShoulderWalk` `EXEC_STATUS`, donkey `0x7100006170`, which runs EVERY FRAME.
///
/// The fourth argument is `lib::L2CValue::NIL` and that is read off the binary rather than off
/// Ghidra's label for it. The instruction is an `adrp`/`ldr` of GOT slot `0x1766c0` followed by
/// the COPY constructor `_ZN3lib8L2CValueC1ERKS0_`, and `disasm --got` names that slot
/// `_ZN3lib8L2CValue3NILE`. Ghidra's export calls the same address `LUA_SCRIPT_LINE_SYSTEM_POST`,
/// which is a real constant (= 1) but not what is there: every genuine lua_const in these
/// functions is a four-byte `ldr w1, [x24, #off]` out of
/// `L2CAgentGeneratedBase::const_value_table__` at slot `0x1766b8`, never a whole-object copy.
/// The same NIL slot is what DK substitutes when `sv_battle_object::module_accessor` returns null
/// in his throw helper, which only makes sense for a nil.
unsafe extern "C" fn shoulder_walk_exec(fighter: &mut L2CFighterCommon) -> L2CValue {
    fighter.sub_walk_uniq_process_main_common(
        L2CValue::new_hash(Hash40::new("shoulder_walk_fast").hash),
        L2CValue::new_hash(Hash40::new("shoulder_walk_middle").hash),
        L2CValue::new_hash(Hash40::new("shoulder_walk_slow").hash),
        L2CValue::new_void(),
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
    if carry_interrupts(fighter, entry) {
        return 1.into();
    }
    if shoulder_pass_check(fighter, entry) {
        return 1.into();
    }

    // DONKEY KONG'S SHOULDERWALK DECISION, donkey 0x7100023cd0, and it is SHORTER than the port's:
    //
    //     if globalTable[0x1a] * lr < get_param_float(common, walk_stick_x)
    //        and globalTable[0x16] == SITUATION_KIND_GROUND      -> SHOULDER_WAIT, true
    //     if globalTable[0x16] == SITUATION_KIND_AIR             -> SHOULDER_FALL, false
    //
    // THERE IS NO TURN BRANCH HERE. The port had one, testing `stick_x * lr < -0.1` -- a
    // threshold from nowhere -- and DK has none: walking with the stick pulled behind you drops
    // below `walk_stick_x`, which returns you to WAIT, and WAIT is where the turn decision lives.
    // Turning straight out of the walk skips the thing that owns that choice.
    //
    // THE OPERATOR IS `<`, NOT `<=`. ShoulderWait uses `<=` for both of its comparisons and
    // ShoulderWalk uses `<` for this one (`_ZNK3lib8L2CValueltERKS0_` at donkey 0x7100023fbc
    // against `_ZNK3lib8L2CValueleERKS0_` at 0x7100024668), so exactly at the threshold he keeps
    // walking. Copied rather than rounded off.
    let stick = stick_x_lr(fighter);
    if stick < common_param_f(boma, WALK_STICK_X)
        && fighter.global_table[0x16] == *SITUATION_KIND_GROUND
    {
        return carry_change_input(fighter, entry, RAT_STATUS_KIND_SHOULDER_WAIT, "SHOULDER_WAIT");
    }
    if fighter.global_table[0x16] == *SITUATION_KIND_AIR {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_FALL, "SHOULDER_FALL");
    }
    0.into()
}

// ---- SHOULDER_TURN ---------------------------------------------------------------------------

unsafe extern "C" fn shoulder_turn_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(fighter, *SITUATION_KIND_GROUND, *FIGHTER_KINETIC_TYPE_TURN, *GROUND_CORRECT_KIND_GROUND, keep_none(), false, false, 0)
}

/// `ShoulderTurn_main`, donkey, transcribed -- and the two lines the port did not have are the
/// entire reason the turn ever worked or did not.
///
///     WorkModule::set_float(get_param_float(common, turn_frame),
///                           FIGHTER_STATUS_TURN_WORK_FLOAT_TURN_FRAME)
///     WorkModule::off_flag(FIGHTER_STATUS_TURN_FLAG_TURN)
///     MotionModule::change_motion(hash40("shoulder_turn"), 0.0, 1.0, false, 0.0, false, false)
///
/// THE PORT FLIPPED HIM BY HAND AND THAT IS A SECOND WRITER. It called
/// `PostureModule::reverse_lr` + `update_rot_y_lr` in the loop on `MotionModule::is_end`, with a
/// comment claiming that "matches DK, whose animation carries the rotation and whose status only
/// commits it once". DK's ShoulderTurn calls neither function. The flip is the ENGINE's, and these
/// two work variables are how it is asked for: `FIGHTER_KINETIC_TYPE_TURN` (already in
/// `shoulder_turn_pre`) plus a `TURN_FRAME` countdown plus `TURN_FLAG_TURN` cleared is the same
/// contract every ordinary Turn status signs. With the engine flipping him AND the port flipping
/// him, the two cancel -- he faces the way he started, `global_table[0x1a] * lr` never changes
/// sign, and the carry can only ever walk in one direction. Which is the report.
unsafe extern "C" fn shoulder_turn_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let frames = common_param_f(boma, TURN_FRAME);
    WorkModule::set_float(boma, frames, *FIGHTER_STATUS_TURN_WORK_FLOAT_TURN_FRAME);
    WorkModule::off_flag(boma, *FIGHTER_STATUS_TURN_FLAG_TURN);
    MotionModule::change_motion(boma, Hash40::new("shoulder_turn"), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_turn_loop as *const () as _))
}

/// `FUN_7100023390`, DK's TURN DRIVER -- the per-frame half of the turn, which the port had none of.
///
/// THIS IS "THE SPRITE PERIODICALLY TURNS AROUND VERY QUICKLY" WHEN CARRYING BACKWARDS, and the
/// measured digest is the whole diagnosis:
///
///     504(=0x1f8) 1f -> 506(=0x1fa) 16f -> 504 1f -> 506 16f -> 504 1f -> 506 16f
///
/// WAIT for one frame, TURN for sixteen -- `shoulder_turn` is a 15-frame motion -- then WAIT for
/// one frame and turn again, forever. `shoulder_turn_main` wrote `common/turn_frame` into
/// `FIGHTER_STATUS_TURN_WORK_FLOAT_TURN_FRAME` and cleared `FIGHTER_STATUS_TURN_FLAG_TURN`, which
/// is the contract -- and then NOTHING COUNTED THE COUNTDOWN DOWN. `lr` never flipped, so the
/// stick still pointed behind him when TURN handed back to WAIT, and WAIT turned him again.
///
/// The driver, from donkey 0x7100023390:
///
///     if arg == 1 then
///         if 0.0 <= get_float(TURN_WORK_FLOAT_TURN_FRAME) then
///             WorkModule::add_float(-1.0, TURN_WORK_FLOAT_TURN_FRAME)
///         elseif not is_flag(TURN_FLAG_TURN) then
///             WorkModule::on_flag(TURN_FLAG_TURN)
///             PostureModule::reverse_lr()
///         end
///     end
///
/// So the flip is ONE event, guarded by its own flag, `turn_frame` frames into the status -- not
/// something to do at `MotionModule::is_end`, which is what the port's deleted hand-rolled version
/// did and got backwards.
///
/// `arg == 1` AND WHERE IT COMES FROM, named because it is the one thing here that is inferred.
/// `ShoulderTurn_main` calls this once with a `false` -- which fails the `== 1` test and does
/// nothing -- and separately assigns `globalTable[0x15] = DAT_71000235b0`, a thunk that calls it
/// with 1. `[0x15]` is the same slot the jumpsquat uses for its own per-frame helper, so it is the
/// status's per-frame line; running the body once per loop is the same work at the same rate.
unsafe fn drive_turn(boma: *mut BattleObjectModuleAccessor) {
    if 0.0 <= WorkModule::get_float(boma, *FIGHTER_STATUS_TURN_WORK_FLOAT_TURN_FRAME) {
        WorkModule::add_float(boma, -1.0, *FIGHTER_STATUS_TURN_WORK_FLOAT_TURN_FRAME);
    } else if !WorkModule::is_flag(boma, *FIGHTER_STATUS_TURN_FLAG_TURN) {
        WorkModule::on_flag(boma, *FIGHTER_STATUS_TURN_FLAG_TURN);
        PostureModule::reverse_lr(boma);
    }
}

/// `ShoulderTurn_main_loop`, donkey: the throw helper, the jump helper, then `is_end` deciding
/// between FALL and WAIT on the situation. Nothing else.
unsafe extern "C" fn shoulder_turn_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    drive_turn(boma);
    hold_victim(boma, entry);

    if !still_held(boma, entry) {
        apply_lift_param(boma, entry, false);
        return carry_change(fighter, entry, *FIGHTER_STATUS_KIND_WAIT, "victim lost -> WAIT");
    }
    if carry_interrupts(fighter, entry) {
        return 1.into();
    }
    if MotionModule::is_end(boma) {
        if fighter.global_table[0x16] == *SITUATION_KIND_AIR {
            return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_FALL, "SHOULDER_FALL");
        }
        if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
            return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_WAIT, "SHOULDER_WAIT");
        }
    }
    0.into()
}

// ---- SHOULDER_FALL / _LANDING ----------------------------------------------------------------

unsafe extern "C" fn shoulder_fall_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(
        fighter,
        *SITUATION_KIND_AIR,
        *FIGHTER_KINETIC_TYPE_MOTION_FALL,
        *GROUND_CORRECT_KIND_AIR,
        // THE ONLY STATUS OF THE TEN THAT KEEPS ANY WORK, and it keeps the FALL work -- which is
        // what carries the fall speed and the fall-special flags across the transition into it.
        (
            *FIGHTER_STATUS_WORK_KEEP_FLAG_FALL_FLAG,
            *FIGHTER_STATUS_WORK_KEEP_FLAG_FALL_INT,
            *FIGHTER_STATUS_WORK_KEEP_FLAG_FALL_FLOAT,
        ),
        false,
        true,
        0,
    )
}

/// `ShoulderFall_main`, donkey: the motion change is CONDITIONAL on where he came from.
///
///     if globalTable[0xa] != FIGHTER_DONKEY_STATUS_KIND_SHOULDER_START then
///         change_motion(hash40("shoulder_fall"), 0.0, 1.0, false, 0.0, false, false)
///     end
///
/// `globalTable[0xa]` is the previous status kind (`[0xb]` is the next one, which `shoulder_exit`
/// already reads). Falling straight out of the lift keeps the lift animation running rather than
/// cutting to the fall pose mid-lift.
unsafe extern "C" fn shoulder_fall_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    if fighter.global_table[0xa].get_i32() != RAT_STATUS_KIND_SHOULDER_START {
        MotionModule::change_motion(boma, Hash40::new("shoulder_fall"), 0.0, 1.0, false, 0.0, false, false);
    }
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_fall_loop as *const () as _))
}

/// `ShoulderFall_main_loop`, donkey:
///
///     if throw_helper() then return 1 end
///     if globalTable[0x16] == SITUATION_KIND_GROUND then
///         change_status(SHOULDER_LANDING, false)
///     elseif MotionModule::is_end() and motion_kind() != hash40("shoulder_fall") then
///         change_motion(hash40("shoulder_fall"), 0.0, 1.0, false, 0.0, false, false)
///     end
///
/// NO JUMP HELPER. He calls it from Wait, Walk and Turn and from none of the airborne statuses --
/// there is no second jump out of a cargo carry.
unsafe extern "C" fn shoulder_fall_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    hold_victim(boma, entry);

    if !still_held(boma, entry) {
        return carry_change(fighter, entry, *FIGHTER_STATUS_KIND_FALL, "victim lost -> FALL");
    }
    if let Some(next) = requested_throw(fighter) {
        return carry_change_input(fighter, entry, next, "requested throw");
    }
    if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_LANDING, "SHOULDER_LANDING");
    }
    if MotionModule::is_end(boma) && MotionModule::motion_kind(boma) != hash40("shoulder_fall") {
        MotionModule::change_motion(boma, Hash40::new("shoulder_fall"), 0.0, 1.0, false, 0.0, false, false);
    }
    0.into()
}

unsafe extern "C" fn shoulder_landing_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(fighter, *SITUATION_KIND_GROUND, *FIGHTER_KINETIC_TYPE_MOTION, *GROUND_CORRECT_KIND_GROUND_OTTOTTO, keep_none(), true, false, 0)
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
    // `ShoulderLanding_main_loop`, donkey: the air test comes FIRST and unconditionally -- you can
    // be knocked off the stage during the landing -- and only then does `is_end` decide.
    if fighter.global_table[0x16] == *SITUATION_KIND_AIR {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_FALL, "SHOULDER_FALL");
    }
    if MotionModule::is_end(boma) {
        let next = if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
            RAT_STATUS_KIND_SHOULDER_WAIT
        } else {
            RAT_STATUS_KIND_SHOULDER_FALL
        };
        return carry_change(fighter, entry, next, "landing over");
    }
    0.into()
}

// ---- THE JUMP OUT OF THE CARRY, AND THE DROP THROUGH A PLATFORM ------------------------------
//
// DK's last four carry statuses, and the reason this port had six of eleven. See
// `shoulder_jump_check` for the helper that reaches them.

/// Both jumpsquats share one `pre`: GROUND, MOTION, `GROUND_CORRECT_KIND_GROUND` -- not the
/// OTTOTTO teeter correction the standing statuses use, because a jumpsquat is not a stance.
unsafe extern "C" fn shoulder_jump_squat_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(
        fighter,
        *SITUATION_KIND_GROUND,
        *FIGHTER_KINETIC_TYPE_MOTION,
        *GROUND_CORRECT_KIND_GROUND,
        keep_none(),
        true,
        false,
        0,
    )
}

/// `FUN_7100022f30`, the short-hop decision, run every frame of the squat by the same function
/// that runs the loop (DK installs entry point `0x7100023150` -- helper then loop -- in
/// `globalTable[0x15]`, and hands `sub_shift_status_main` entry point `0x71000231b0`, which is the
/// loop alone).
///
///     if globalTable[9] == FIGHTER_DONKEY_STATUS_KIND_SHOULDER_JUMP_SQUAT_B then
///         if ControlModule::check_button_off(CONTROL_PAD_BUTTON_JUMP) then
///             WorkModule::on_flag(FIGHTER_STATUS_WORK_ID_FLAG_RESERVE_JUMP_MINI)
///         end
///     elseif ControlModule::get_stick_y() < get_param_float(common, jump_neutral_y) then
///         WorkModule::on_flag(FIGHTER_STATUS_WORK_ID_FLAG_RESERVE_JUMP_MINI)
///     end
///
/// A DEVIATION, NAMED: `globalTable[0x15]`'s role is not identified here, so rather than write a
/// slot whose meaning is a guess, the loop below calls this itself once per frame. That is what
/// installing `0x7100023150` achieves -- the two entry points differ by exactly this call -- and
/// running the superset entry once a frame is the same work in the same order. What it does not
/// reproduce is whatever else `[0x15]` is read by, which is why it is written down here.
unsafe fn reserve_jump_mini(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    if fighter.global_table[9].get_i32() == RAT_STATUS_KIND_SHOULDER_JUMP_SQUAT_B {
        if ControlModule::check_button_off(boma, *CONTROL_PAD_BUTTON_JUMP) {
            WorkModule::on_flag(boma, *FIGHTER_STATUS_WORK_ID_FLAG_RESERVE_JUMP_MINI);
        }
    } else if ControlModule::get_stick_y(boma) < common_param_f(boma, JUMP_NEUTRAL_Y) {
        WorkModule::on_flag(boma, *FIGHTER_STATUS_WORK_ID_FLAG_RESERVE_JUMP_MINI);
    }
}

/// `FUN_7100022d00`, the `main` both jumpsquats share.
unsafe extern "C" fn shoulder_jump_squat_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    MotionModule::change_motion(
        boma,
        Hash40::new("shoulder_jump_squat"),
        0.0,
        1.0,
        false,
        0.0,
        false,
        false,
    );
    if !StopModule::is_stop(boma) {
        reserve_jump_mini(fighter);
    }
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_jump_squat_loop as *const () as _))
}

/// `0x7100023150`/`0x71000231b0`, the jumpsquat loop:
///
///     reserve_jump_mini()                                  -- the [0x15] entry point's extra call
///     if ControlModule::is_jump_mini_button() then
///         WorkModule::on_flag(FIGHTER_STATUS_WORK_ID_FLAG_RESERVE_JUMP_MINI)
///     end
///     if MotionModule::is_end() then change_status(SHOULDER_JUMP, true)
///     elseif globalTable[0x16] == SITUATION_KIND_AIR then change_status(SHOULDER_FALL, true) end
///     if throw_helper() then return 1 end
unsafe extern "C" fn shoulder_jump_squat_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    hold_victim(boma, entry);

    if !still_held(boma, entry) {
        apply_lift_param(boma, entry, false);
        return carry_change(fighter, entry, *FIGHTER_STATUS_KIND_WAIT, "victim lost -> WAIT");
    }
    reserve_jump_mini(fighter);
    if ControlModule::is_jump_mini_button(boma) {
        WorkModule::on_flag(boma, *FIGHTER_STATUS_WORK_ID_FLAG_RESERVE_JUMP_MINI);
    }
    if MotionModule::is_end(boma) {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_JUMP, "SHOULDER_JUMP");
    }
    if fighter.global_table[0x16] == *SITUATION_KIND_AIR {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_FALL, "SHOULDER_FALL");
    }
    if let Some(next) = requested_throw(fighter) {
        return carry_change_input(fighter, entry, next, "requested throw");
    }
    0.into()
}

/// `ShoulderJump_pre`, donkey: AIR, `FIGHTER_KINETIC_TYPE_JUMP`, `GROUND_CORRECT_KIND_AIR`.
///
/// JUMP is the kinetic type that reads the fighter's own jump params -- Pikachu's `jump_y`,
/// `jump_speed_x_max` and the rest -- so the height of a cargo jump is Pikachu's height and not a
/// number written here. That is the same relationship `sub_walk_uniq_process_*_common` gives the
/// walk.
unsafe extern "C" fn shoulder_jump_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(
        fighter,
        *SITUATION_KIND_AIR,
        *FIGHTER_KINETIC_TYPE_JUMP,
        *GROUND_CORRECT_KIND_AIR,
        keep_none(),
        false,
        true,
        0,
    )
}

unsafe extern "C" fn shoulder_jump_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    MotionModule::change_motion(boma, Hash40::new("shoulder_jump"), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_jump_loop as *const () as _))
}

/// `ShoulderJump_main_loop`, donkey, which is two tests:
///
///     if throw_helper() then return 1 end
///     if MotionModule::is_end() then change_status(SHOULDER_FALL, false) end
unsafe extern "C" fn shoulder_jump_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    CARRY_FRAMES[entry] += 1;
    hold_victim(boma, entry);

    if !still_held(boma, entry) {
        return carry_change(fighter, entry, *FIGHTER_STATUS_KIND_FALL, "victim lost -> FALL");
    }
    if let Some(next) = requested_throw(fighter) {
        return carry_change_input(fighter, entry, next, "requested throw");
    }
    if MotionModule::is_end(boma) {
        return carry_change(fighter, entry, RAT_STATUS_KIND_SHOULDER_FALL, "SHOULDER_FALL");
    }
    0.into()
}

/// `ShoulderPass_pre`, donkey: AIR, MOTION_FALL, and `GROUND_CORRECT_KIND_KEEP` -- KEEP rather
/// than AIR because `main` is about to do the correction itself, in the right order relative to
/// `pass_floor`.
unsafe extern "C" fn shoulder_pass_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    shoulder_pre(
        fighter,
        *SITUATION_KIND_AIR,
        *FIGHTER_KINETIC_TYPE_MOTION_FALL,
        *GROUND_CORRECT_KIND_KEEP,
        keep_none(),
        false,
        true,
        0,
    )
}

/// `ShoulderPass_main`, donkey, four calls in this order and the order is the whole function:
///
///     ControlModule::reset_flick_y()
///     GroundModule::pass_floor()
///     self:set_situation(SITUATION_KIND_AIR)
///     GroundModule::correct(GROUND_CORRECT_KIND_AIR)
///
/// `pass_floor`'s result is constructed into an L2CValue and immediately discarded in his code, so
/// it is not a condition -- the drop is unconditional once the status is entered, and the status
/// is only entered after `GroundModule::is_passable_check` said yes. `reset_flick_y` FIRST is what
/// stops the same downward flick being read again next frame as another drop.
///
/// DK then shifts to a loop that is the same function as `ShoulderFall_main` -- Ghidra folded them
/// -- so the pass hands straight over to the fall, which is what the motion and the kinetic type
/// already say it is.
unsafe extern "C" fn shoulder_pass_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    ControlModule::reset_flick_y(boma);
    GroundModule::pass_floor(boma);
    fighter.set_situation(L2CValue::new_int(*SITUATION_KIND_AIR as u64));
    GroundModule::correct(boma, smash::app::GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
    fighter.sub_shift_status_main(L2CValue::Ptr(shoulder_fall_main as *const () as _))
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

/// `ThrowFF_init`, and it IS an init line -- `tools/check-statuslines.sh` found all four throws
/// with an `init.c` in the decomp and no `Init` registered here.
///
/// THE BODY WAS ALREADY HERE, IN `main`, WHICH IS ONE STATUS LINE TOO LATE. `pre`, then `init`,
/// then `main`: telling the victim they have been thrown is the first thing DK does in the status,
/// and doing it from `main` means the throw's own first frame has already run. It also has to
/// happen before `game_throwff`'s `ATTACK_ABS` on frame 0 has anything to resolve against.
///
/// AND IT READS THE MOTION OUT OF THE WORK INT RATHER THAN OFF A STRING WE PASSED IN.
/// `ShoulderStart_exit` wrote the throw's motion name into
/// `FIGHTER_STATUS_CATCH_WAIT_WORK_INT_MOTION_KIND` (that is `leave_carry`), and DK's init reads it
/// straight back to pick the victim's thrown animation. The port had a `thrown_motion(&str)` doing
/// the same mapping off a hardcoded argument, which worked but left the work int written and
/// unread -- a loop the source closes and this port did not.
///
/// The target object is the LINK's node id for the same reason: it is what DK reads, and it cannot
/// disagree with the engine the way our own `VICTIM[entry]` can.
unsafe extern "C" fn throw_init(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    let kind = WorkModule::get_int64(boma, *FIGHTER_STATUS_CATCH_WAIT_WORK_INT_MOTION_KIND);
    let thrown = if kind == Hash40::new("throw_f_f").hash {
        "thrown_f_donkey_f"
    } else if kind == Hash40::new("throw_f_b").hash {
        "thrown_f_donkey_b"
    } else if kind == Hash40::new("throw_f_hi").hash {
        "thrown_f_donkey_hi"
    } else {
        "thrown_f_donkey_lw"
    };
    WorkModule::set_int64(boma, 0, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_GROUP);
    WorkModule::set_int64(boma, 0, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_NO);
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
        let them = LinkModule::get_node_object_id(boma, *LINK_NO_CAPTURE);
        WorkModule::set_int64(boma, them as u64, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT);
    } else {
        // No link means no engine-side throw target, and `game_throwff`'s `ATK_HIT_ABS` on frame
        // 15 reads this. Our own bookkeeping is the fallback, which is the ADR 0004 shape again:
        // the precondition DK gets for free has to be supplied here.
        WorkModule::set_int64(
            boma,
            VICTIM[entry] as u64,
            *FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT,
        );
    }
    crate::telemetry::probe::event(format!(
        "[rat] SHOULDER p{entry} throw init: motion_kind={kind:#x} -> victim plays {thrown}, \
         target={:#x}; {}",
        WorkModule::get_int64(boma, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT),
        crate::telemetry::probe::ctx(boma)
    ));
    0.into()
}

 unsafe fn throw_main(fighter: &mut L2CFighterCommon, motion: &str) -> L2CValue {
    let boma = fighter.module_accessor;
    MotionModule::change_motion(boma, Hash40::new(motion), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(throw_loop as *const () as _))
}

unsafe extern "C" fn throw_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    // DK'S OWN THROW LOOP, `FUN_7100021af0`, AND IT IS THE ANSWER TO "TOO MUCH ENDING LAG".
    //
    //     if CancelModule::is_enable_cancel() then
    //         if globalTable[0x16] == SITUATION_KIND_GROUND then
    //             if sub_wait_ground_check_common(false) then return 1 end
    //         elseif globalTable[0x16] == SITUATION_KIND_AIR then
    //             if sub_air_check_fall_common() then return 1 end
    //         end
    //     end
    //
    // THE PORT WAITED FOR `MotionModule::is_end` INSTEAD, and the motion list says what that
    // costs. Every one of the four throws has a first-actionable frame well inside its animation:
    //
    //     throw_f_f   faf 46   animation 49
    //     throw_f_b   faf 43   animation 59
    //     throw_f_hi  faf 30   animation 49      <- nineteen frames of invented end lag
    //     throw_f_lw  faf 45   animation 59
    //
    // `CancelModule::is_enable_cancel` is how the engine reports that the FAF has passed, which is
    // why it is the FIRST call in DK's loop -- and `sub_wait_ground_check_common` /
    // `sub_air_check_fall_common` are the common helpers that then hand control back. A brand-new
    // status id gets none of that for free: 0x1FD is not in the engine's throw family, so nothing
    // was reading the FAF at all.
    if CancelModule::is_enable_cancel(boma) {
        if fighter.global_table[0x16] == *SITUATION_KIND_GROUND {
            if fighter.sub_wait_ground_check_common(false.into()).get_bool() {
                return 1.into();
            }
        } else if fighter.global_table[0x16] == *SITUATION_KIND_AIR
            && fighter.sub_air_check_fall_common().get_bool()
        {
            return 1.into();
        }
    }
    // The animation running out is still a backstop, for the case where neither common helper
    // takes the frame -- a situation that is neither GROUND nor AIR, which is how the port used to
    // lose a whole carry at a ledge.
    if MotionModule::is_end(boma) {
        let ground = StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND;
        let next = if ground { *FIGHTER_STATUS_KIND_WAIT } else { *FIGHTER_STATUS_KIND_FALL };
        return carry_change(fighter, entry, next, "throw animation over");
    }
    0.into()
}

/// `ThrowFF_exit` / `ThrowFB_exit` / `ThrowFHi_exit` / `ThrowFLw_exit`, donkey -- all four are the
/// same folded function and its entire body is one call:
///
///     CatchModule::catch_cut(boma, false, false)
///
/// THIS IS "AFTER ONE CARGO THROW I CAN'T GRAB THE OPPONENT AGAIN UNTIL AN L+R+A RESET."
///
/// `enter_carry` records a capture and links both ends, which is what makes the carry work at all.
/// Nothing undid it. `shoulder_exit` clears `ignore_catching` on the way into a throw, and the
/// throw then ran to completion with the engine still holding a capture record naming Pikachu as
/// the master of a fighter he had already thrown -- so the next catch had nowhere to go. The
/// engine's own answer is this one line on the way OUT of the throw, and it was the only carry
/// status without an `Exit` registered.
///
/// Registered on the four throws rather than called from `throw_loop`, for the reason
/// `carry_change`'s comment already gives about `leave_carry`: an exit line runs when the engine
/// ends the status too, and getting hit out of a cargo throw is exactly that case.
/// THE THROW PATH WAS THE ONE THAT NEVER UNLINKED, and the playtest bisected it for me: the
/// regrab works after the victim mashes out and not after a throw. The mash-out goes through
/// `cut_capture`, which unlinks; this went through a bare `catch_cut`, which does not.
/// THE WHOLE TEARDOWN LIVES HERE NOW, and the reason is the same one `carry_change`'s comment
/// gives about `leave_carry`: the loop above can be left by the engine -- getting hit out of a
/// throw was already measured doing exactly that -- and an exit line runs either way. It used to
/// be split between here and `throw_loop`'s `is_end` branch, so an interrupted throw kept the
/// victim on the books until the hold ceiling fired.
///
/// `drop_capture` runs BEFORE `forget`, because it needs to know who the victim was to clear the
/// engine's stale capture record off them.
unsafe extern "C" fn throw_exit(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    crate::victim::drop_capture(entry, "throw over");
    crate::victim::forget(entry, "throw over");
    VICTIM[entry] = *BATTLE_OBJECT_ID_INVALID as u32;
    apply_lift_param(boma, entry, false);
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
        .status(Exec, RAT_STATUS_KIND_SHOULDER_WALK,    shoulder_walk_exec)
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_JUMP_SQUAT,   shoulder_jump_squat_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_JUMP_SQUAT,   shoulder_jump_squat_main)
        .status(Exit, RAT_STATUS_KIND_SHOULDER_JUMP_SQUAT,   shoulder_exit)
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_JUMP_SQUAT_B, shoulder_jump_squat_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_JUMP_SQUAT_B, shoulder_jump_squat_main)
        .status(Exit, RAT_STATUS_KIND_SHOULDER_JUMP_SQUAT_B, shoulder_exit)
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_JUMP,    shoulder_jump_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_JUMP,    shoulder_jump_main)
        .status(Exit, RAT_STATUS_KIND_SHOULDER_JUMP,    shoulder_exit)
        .status(Pre,  RAT_STATUS_KIND_SHOULDER_PASS,    shoulder_pass_pre)
        .status(Main, RAT_STATUS_KIND_SHOULDER_PASS,    shoulder_pass_main)
        .status(Exit, RAT_STATUS_KIND_SHOULDER_PASS,    shoulder_exit)
        .status(Init, RAT_STATUS_KIND_THROW_F_F, throw_init)
        .status(Pre,  RAT_STATUS_KIND_THROW_F_F,  throw_pre)
        .status(Main, RAT_STATUS_KIND_THROW_F_F,  throw_ff_main)
        .status(Exit, RAT_STATUS_KIND_THROW_F_F,  throw_exit)
        .status(Init, RAT_STATUS_KIND_THROW_F_B, throw_init)
        .status(Pre,  RAT_STATUS_KIND_THROW_F_B,  throw_pre)
        .status(Main, RAT_STATUS_KIND_THROW_F_B,  throw_fb_main)
        .status(Exit, RAT_STATUS_KIND_THROW_F_B,  throw_exit)
        .status(Init, RAT_STATUS_KIND_THROW_F_HI, throw_init)
        .status(Pre,  RAT_STATUS_KIND_THROW_F_HI, throw_pre)
        .status(Main, RAT_STATUS_KIND_THROW_F_HI, throw_fhi_main)
        .status(Exit, RAT_STATUS_KIND_THROW_F_HI, throw_exit)
        .status(Init, RAT_STATUS_KIND_THROW_F_LW, throw_init)
        .status(Pre,  RAT_STATUS_KIND_THROW_F_LW, throw_pre)
        .status(Main, RAT_STATUS_KIND_THROW_F_LW, throw_flw_main)
        .status(Exit, RAT_STATUS_KIND_THROW_F_LW, throw_exit)
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
    // `catch_cut` and `set_ignore_catching(false)` are DK's two lines here; the unlink is ours to
    // undo because the link was ours to make. See `crate::victim::drop_capture`.
    crate::victim::drop_capture(entry, "carry released");
}

/// `carry_change` for the transitions DK makes with `change_status(kind, TRUE)`.
///
/// THE SECOND ARGUMENT TO `change_status` IS NOT ALWAYS FALSE, AND THIS PORT PASSED FALSE FOR ALL
/// OF THEM. Its meaning is not established here -- `L2CFighterBase::change_status` (common
/// 0x710000a1e0) does nothing but forward it to whatever callable sits in `globalTable[0x10]`, so
/// the semantics are in the engine and out of reach of this repo's tooling. skyline-smash calls
/// it `arg2` and so does this comment.
///
/// What IS established is every value DK passes, read off his binary one `mov w1` at a time:
///
///     ShoulderWait  -> PASS true   TURN true   WALK true   FALL false
///     ShoulderWalk  -> PASS true   WAIT true   FALL false
///     ShoulderTurn  -> FALL false  WAIT false
///     ShoulderFall  -> LANDING false
///     ShoulderLand  -> FALL false  WAIT false
///     ShoulderJump  -> FALL false
///     JumpSquat     -> JUMP false  FALL false
///     jump helper   -> JUMP_SQUAT true   JUMP_SQUAT_B true
///     throw helper  -> THROW_F_F/_B/_HI/_LW true
///
/// The pattern that falls out is that `true` goes with a transition the PLAYER asked for this
/// frame -- a stick crossing a threshold, a button -- and `false` with one the world or the
/// animation forced: a situation change, a motion ending. Walk -> Wait is `true` because the stick
/// dropping below `walk_stick_x` is still an input; Turn -> Wait is `false` because it is the turn
/// animation finishing. That reading is a guess and the values are not: the values are copied.
///
/// The port's own substituted transitions -- `victim lost -> WAIT` and the end of a throw -- have
/// no counterpart in DK and keep `false`.
unsafe fn carry_change_input(
    fighter: &mut L2CFighterCommon,
    entry: usize,
    next: i32,
    why: &str,
) -> L2CValue {
    if !crate::victim::allow_shift(entry, why) {
        return 0.into();
    }
    fighter.change_status(next.into(), true.into());
    1.into()
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

/// The statuses that mean "still carrying", so `shoulder_exit` can tell a carry-to-carry
/// transition from losing the carry. DK tests all eleven of his, in the order his exit lists them;
/// this is now all ten of ours plus the four throws he also allows through.
fn is_carry_status(kind: i32) -> bool {
    kind == RAT_STATUS_KIND_SHOULDER_START
        || kind == RAT_STATUS_KIND_SHOULDER_WAIT
        || kind == RAT_STATUS_KIND_SHOULDER_WALK
        || kind == RAT_STATUS_KIND_SHOULDER_TURN
        || kind == RAT_STATUS_KIND_SHOULDER_JUMP_SQUAT
        || kind == RAT_STATUS_KIND_SHOULDER_JUMP_SQUAT_B
        || kind == RAT_STATUS_KIND_SHOULDER_JUMP
        || kind == RAT_STATUS_KIND_SHOULDER_FALL
        || kind == RAT_STATUS_KIND_SHOULDER_LANDING
        || kind == RAT_STATUS_KIND_SHOULDER_PASS
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
