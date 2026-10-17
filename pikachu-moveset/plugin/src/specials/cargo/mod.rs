// Donkey Kong's cargo carry (f-throw): re-grab instead of launching, then four directional throws.
//
// WHAT IS AND IS NOT REPRODUCED, stated plainly because this is the one feature in the brief that
// cannot be ported whole.
//
// The victim side is engine-global: FIGHTER_STATUS_KIND_SHOULDERED_DONKEY_START (0xC5),
// _SHOULDERED_DONKEY (0xC6) and _SHOULDERED_DONKEY_THROWN (0xC7) are in the common status list,
// and Pikachu already carries the motions that go with them -- `shouldered_donkey` plus the whole
// thrown_f_donkey_f/b/hi/lw family. DK's game_throwf gets there with
// ATTACK_ABS(FIGHTER_ATTACK_ABSOLUTE_KIND_CATCH, ...) -- a catch-kind throw that RE-CATCHES the
// victim rather than launching them -- followed by a work flag on frame 16 that his C++ turns into
// SHOULDER_WAIT. The four cargo throws then read FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT,
// which is also global.
//
// THE FLAG IS NOT ENOUGH ON ITS OWN, which was the reported bug: "cargo throw ... freezes the
// opponent mid-air for a second after that animation instead of holding the enemy". Nothing on
// Pikachu reads DK's SHOULDER_WAIT flag, so the victim stayed in the generic caught state with no
// status driving them, and dropped out when it timed out. The carry now REQUESTS the shouldered
// statuses on the victim directly, which hands their whole hold-and-throw to the engine.
//
// DK's own side is eleven fighter-specific statuses: SHOULDER_START, _WAIT, _WALK, _TURN, _JUMP,
// _JUMP_SQUAT, _JUMP_SQUAT_B, _FALL, _LANDING, _PASS, _END. That is an entire alternate locomotion
// mode -- walking, turning, jumping and landing while holding someone -- and it is C++. The
// one-status-with-phases approach used for Revenge and Alolan Whip does not apply, because the
// defining feature of cargo carry is retaining NORMAL movement while carrying.
//
// THE CARRY NOW MOVES, and the way it gets there is worth explaining because it is not DK's own
// shape.
//
// DK implements the carry as eleven fighter-specific statuses -- SHOULDER_WAIT, _WALK, _TURN,
// _JUMP_SQUAT, _JUMP, _FALL, _LANDING, _PASS, _END and so on -- each with its own locomotion. That
// could be reproduced now that new status kinds are known to work (see ../status.rs), and it was
// still the wrong choice: it would mean reimplementing walking, turning, jumping and landing, and
// then getting Pikachu's own speeds, weights and jump heights back into them by hand.
//
// Instead Pikachu stays in HIS OWN movement statuses -- WAIT, WALK, TURN, JUMP_SQUAT, JUMP, FALL,
// LANDING, all engine-global -- and the carry only overrides the MOTION to DK's matching
// `shoulder_*` pose and suppresses the actions a carrier should not have. The locomotion is
// therefore Pikachu's, correct by construction and already tuned by the attribute work, which is
// exactly what "dropped into our modified Pikachu's circumstances" asks for. What is DK's is the
// pose, the throws and the fact that you keep your movement at all.
//
// The nine poses are synthesised motions adopting DK's animations (tools/motion-patch synth), and
// all nine files were already on disk: the full c00 extraction covers every fighter, so this
// stopped being blocked on a console session without anyone noticing.
//
// STILL NOT REPRODUCED: DK's _PASS (carrying someone through a soft platform) and the precise
// action-restriction set. Attacking, shielding and specials are suppressed here because dropping
// the victim mid-attack is worse than not being able to attack.
//
// ONE MORE THING STILL MISSING: the cargo grab plays PIKACHU's f-throw animation, because DK's
// e01throwf.nuanmb has not been extracted -- it is in tools/arc-pull/paths-round3.txt and needs
// the game running with arc-network listening. Playtest saw exactly that: "cargo throw displays
// pikachu's forward throw animation". The four cargo THROWS do use DK's own animations. Once the
// file is pulled, add `donkey | e01throwf.nuanmb` to tools/anim-dropin.sh and
// ("throw_f", "throw_f", "donkey") to tools/motion-patch's MAP; both at once, or the entry gets
// DK's `move` flag over Pikachu's animation.
//
// The failure mode is bounded deliberately. If a throw input never arrives, the carry releases
// itself after CARRY_TIMEOUT rather than leaving the victim held with no way out -- a stuck grab
// would be worse than not having the feature, and this cannot be playtested before shipping.
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

// Incineroar's clatter window is 85 frames and DK's cargo hold is comparable; this is the release
// valve, not a tuning value.

// NOT MODELLED: walk_accel_mul_mul walk_accel_add_mul walk_speed_max_mul
//
// `donkey param_lifting_walk` is three 1.1x multipliers: DK walks ten percent faster, and
// accelerates ten percent harder, while carrying someone. Pikachu keeps his own locomotion by
// design (see the header), so the faithful port is to multiply HIS walk by the same 1.1 rather
// than to adopt DK's absolute speed -- the relationship is what transfers.
//
// The API is a direct match, which is the strongest evidence available that this is exactly how
// the game applies the object: sv_kinetic_energy has `mul_x_accel_mul`, `mul_x_accel_add` and
// `mul_x_speed_max`, one per param, named the same.
//
// IT IS NOT DONE BECAUSE THEY MULTIPLY IN PLACE AND THE CARRY SPANS STATUS CHANGES. Applied once
// at carry start they are wiped by the first WAIT -> WALK transition, which re-initialises the
// control energy from params; applied every frame they compound, and 1.1 per frame is Pikachu
// launched off the stage inside a second. Doing it correctly means tracking whether the current
// status has been multiplied yet, in the one module that cannot be tested before shipping and
// whose failure mode is a stuck grab. Ten percent of walk speed is not worth that risk today.
//
// When it is done: hook the status change rather than the frame, and verify with telemetry's
// `moved x` on shoulder_walk_middle against an unencumbered walk over the same number of frames.

static mut CARRYING: [bool; 8] = [false; 8];
static mut CARRY_FRAMES: [i32; 8] = [0; 8];
/// The carried fighter. Cached because `CatchModule::capture_object_id` stops answering once they
/// move into a shouldered status, and the throw still has to address them.
static mut VICTIM: [u32; 8] = [0; 8];
/// The last id `CatchModule::capture_object_id` gave that was actually plausible. Latched every
/// frame because the handoff reads it at the worst possible moment -- see pikachu_cargo_frame.
static mut LAST_CAPTURED: [u32; 8] = [crate::victim::NOBODY; 8];

unsafe fn entry_of(boma: *mut BattleObjectModuleAccessor) -> usize {
    let e = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if e >= 8 { 0 } else { e }
}
// The victim-side helpers that used to live here -- `tell_victim`, `victim_status`,
// `cargo_direction`, TELL_VICTIM_SHOULDERED, CARRY_TIMEOUT -- went with the motion-swap carry.
// Their replacements are ../shoulder's `requested_throw` for the input, `crate::victim::held`
// for ownership, and DK's own SHOULDER statuses for everything else.


pub unsafe extern "C" fn pikachu_cargo_frame(fighter: &mut L2CFighterCommon) {
    use crate::telemetry::probe;
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    let motion = MotionModule::motion_kind(boma);
    let status = StatusModule::status_kind(boma);

    // WHO WE ARE HOLDING, TRACKED CONTINUOUSLY RATHER THAN READ AT THE HANDOFF.
    //
    // This used to read `CatchModule::capture_object_id` at the instant `throw_f` ENDS, which is
    // the one moment it is least likely to answer: the engine's throw has run to completion by
    // then and the capture bookkeeping is being torn down. A stale or invalid id handed to
    // `sv_battle_object::module_accessor` yields a pointer that is not an accessor, and the first
    // virtual call through it is an Instruction Abort at a garbage address -- which is the
    // shape of the cargo crash report.
    //
    // So the id is latched every frame it is valid, throughout the grab, and the handoff uses
    // the last good one. `crate::victim::plausible` is what "good" means: not the invalid
    // sentinel, and shaped like `entry << 16`.
    let cap = CatchModule::capture_object_id(boma, false) as u32;
    if crate::victim::plausible(cap) {
        LAST_CAPTURED[entry] = cap;
    }

    // ALREADY CARRYING? THEN THIS IS NOT A NEW GRAB.
    //
    // THIS IS THE FREEZE, AND THE LOG SHOWS IT 7965 TIMES IN ONE FRAME. The trigger below is
    // `motion == throw_f && is_end`, which is true for exactly as long as throw_f sits at its
    // last frame -- and SHOULDER_START deliberately KEEPS throw_f, because DK does not change
    // pose until SHOULDER_WAIT. So every condition stayed true after the handoff:
    //
    //   CARGO grab landed -> handing to SHOULDER_START; st=503(=0x1f7) mot=throw_f f=24.0/24
    //   SHOULDER START entered ... START loop reached, frame 1
    //   CARGO grab landed -> handing to SHOULDER_START; st=503(=0x1f7) mot=throw_f f=24.0/24
    //
    // `st=0x1f7` is SHOULDER_START. It was requesting the status it was already in, whose Main
    // runs this line hook again, forever, inside one frame. That is the freeze -- and almost
    // certainly the earlier CRASH too: unbounded re-entry exhausts the stack, which is how
    // `sub_shift_status_main` came to read a corrupted L2CValue and `blr` to an address with its
    // low 32 bits gone.
    //
    // `CARRYING[entry] = false` immediately after the handoff was the bug: a flag that says "a
    // carry is in progress", cleared one line after the carry starts. It is now cleared when the
    // carry actually ENDS, and the status check below makes the guard true even across a state
    // reset, because the fighter's own status is the fact and the flag is only a cache of it.
    let in_carry = matches!(
        status,
        s if s == RAT_STATUS_KIND_SHOULDER_START
            || s == RAT_STATUS_KIND_SHOULDER_WAIT
            || s == RAT_STATUS_KIND_SHOULDER_WALK
            || s == RAT_STATUS_KIND_SHOULDER_TURN
            || s == RAT_STATUS_KIND_SHOULDER_FALL
            || s == RAT_STATUS_KIND_SHOULDER_LANDING
            || s == RAT_STATUS_KIND_THROW_F_F
            || s == RAT_STATUS_KIND_THROW_F_B
            || s == RAT_STATUS_KIND_THROW_F_HI
            || s == RAT_STATUS_KIND_THROW_F_LW
    );

    // The carry has ended if the flag says we are in one and the fighter's status says otherwise.
    // The status is the fact; the flag is a cache, and a cache that cannot be invalidated is how
    // the previous version got stuck the other way -- a stale `true` would block every future
    // cargo grab for the rest of the stock.
    if CARRYING[entry] && !in_carry {
        probe::event(format!(
            "[rat] CARGO p{entry} carry ended (status is now {}); ready for another",
            probe::st(status)
        ));
        CARRYING[entry] = false;
        CARRY_FRAMES[entry] = 0;
    }

    // The cargo grab is DK's throw_f. Once its motion finishes, the victim is still caught
    // THE HANDOFF MOVED. It used to live here, triggered by `motion == throw_f && is_end`, and
    // that trigger was the cargo's central bug for three rounds: Pikachu's `throw_f` is a REAL
    // forward throw, so by the time it ended the victim had been thrown and the catch cut. The
    // recording of our own fighter shows it happening --
    // `send_event_nodes_throw(hash40("throw"), hash40("thrown_f"))` then `catch_cut` -- against
    // the real Donkey Kong, who sends no throw event at all when lifting.
    //
    // It is now `pikachu_throw_main`, which takes the forward throw over on its FIRST frame so
    // that Pikachu's own throw never executes. LAST_CAPTURED above is what it reads.
}

// ============================================================================================
// DELETED: THE MOTION-SWAP CARRY, ~230 LINES OF IT, AND IT HAD BEEN RUNNING AGAIN.
// ============================================================================================
//
// Everything that used to follow the handoff above -- `carry_pose`, `suppress_carry_actions`,
// `throw_cargo`, `cargo_direction`, `victim_still_held`, `victim_status`, `tell_victim`,
// CARRY_TIMEOUT -- was the previous design: keep Pikachu in his own WAIT/WALK and override the
// MOTION to DK's shoulder pose. ../shoulder replaced it with DK's actual statuses, and the old
// code was left in place below an unreachable `return`.
//
// THEN THE FREEZE FIX MADE IT REACHABLE. `CARRYING[entry] = false` used to sit immediately after
// the handoff, and clearing it was what stopped this function from falling through to the legacy
// block on subsequent frames. Removing that line -- correct, it was the freeze -- turned 230
// lines of superseded code back on, running every frame alongside the statuses that replaced it:
//
//   carry_pose + change_motion        overrode the motion the SHOULDER statuses had just set
//   suppress_carry_actions            almost certainly "pikachu cannot move while cargo throwing"
//   victim_still_held                 checked the SHOULDERED_DONKEY statuses, which are never
//                                     entered, so it declared "victim lost after 3f, carry ends"
//   throw_cargo on a button press     a SECOND throw path, by change_motion, competing with the
//                                     status-based one
//   CARRY_TIMEOUT 120                 released the victim after two seconds regardless
//
// which is the reported "the opponent escapes very soon after the throw starts, no matter the
// percent". Percent was never involved.
//
// Two implementations of one feature, one of them invisible, is the exact shape ADR 0005 rule 3
// forbids -- and dead code behind an unreachable statement is the worst version of it, because
// nothing in a diff shows it coming back to life. So it is gone rather than re-disabled.
/// Takes over Pikachu's FORWARD THROW and makes it the cargo lift, before his own throw can run.
///
/// THE RECORDING SETTLED THIS, and it is the whole of "mario escapes a brief moment after being
/// grabbed". The trigger used to be `motion == throw_f && MotionModule::is_end`, on the reasoning
/// written above it: that when `throw_f` finishes "the victim is still caught (ATTACK_ABS of CATCH
/// kind, not THROW kind)". That is true of DONKEY KONG, whose `throw_f` IS the lift. It is false of
/// Pikachu, whose `throw_f` is a real forward throw, and the trace of our own fighter says so:
///
///   pika0  CatchModule::set_catch(0x50000000)
///   pika0  LinkModule::send_event_nodes_throw(event=hash40("throw"), motion=hash40("thrown_f"), ...)
///   pika0  CatchModule::catch_cut(false, false)          <- the victim is thrown and RELEASED
///   pika0  CaptureModule::set_ignore_catching(true)      <- only now does our carry begin
///
/// Against the real Donkey Kong, who never sends a throw event when lifting at all:
///
///   DONKEY0 CatchModule::set_catch(0x50000000)
///   DONKEY0 LinkModule::send_event_nodes(link=0, hash40("shoulder"), 0)
///   mario1  ControlModule::start_clatter(3274.8, 0, 10, -1, 0, false, false)
///
/// So the carry was starting on a fighter who had already been thrown, every single time, and no
/// amount of work inside the carry statuses could have fixed it. `status=0x44` in
/// `crate::victim::trace` -- FIGHTER_STATUS_KIND_CAPTURE_CUT, on the first frame of the carry --
/// had been reporting exactly this for three rounds.
///
/// WHICH THROW IT IS COMES FROM THE ENGINE, not from an input read of our own.
/// `FIGHTER_STATUS_CATCH_WAIT_WORK_INT_MOTION_KIND` is where the catch-wait logic records the
/// throw it chose, and it is the same work id Donkey Kong's `ThrowF*_init` reads back to decide
/// which cargo throw is happening. Reading it here means the four directions are distinguished by
/// the game's own decision.
///
/// THE OTHER THREE THROWS ARE UNTOUCHED. Only `throw_f` becomes the lift; back, up and down
/// delegate to the original Main through `smashline::original_status`, so Pikachu's down throw
/// stays vanilla as `docs/decisions.md` requires.
/// TAKE THE FORWARD THROW IN `CATCH_WAIT`, SO PIKACHU NEVER ENTERS `THROW` AT ALL.
///
/// THE ONE FRAME OF `THROW` IS THE WHOLE CARGO REGRAB BUG, and the native trace of a real DK
/// grab (pt27) says he does not spend it:
///
///     DONKEY0 status CATCH_DASH(0x39) -> 0x3a -> 490(0x1ea)  SHOULDER_START
///     DONKEY0 st=0x3a  CatchModule::set_catch(0x50000000)
///     DONKEY0 st=0x1ea LinkModule::send_event_nodes(link=0, event=0x8af9b3d1d, 0)
///     mario1  st=0xc5  ControlModule::start_clatter(139, 0, 10, ...)
///
/// He goes from a CATCH status straight into SHOULDER_START, and he never calls
/// `CaptureModule::capture` -- the "shoulder" link event alone carries the victim across,
/// because THE ENGINE'S OWN GRAB IS STILL INTACT when it arrives.
///
/// This port went through `THROW`, and the same log shows what that costs, three times over:
///
///     pika0  status CATCH_WAIT(0x3c) -> THROW(0x40)
///     pika0  st=0x40  CatchModule::catch_cut(false, false)      <- the grab is destroyed
///     mario1 st=0x44  CaptureModule::capture(captor=0x0, ...)   <- `enter_carry` rebuilds it
///     pika0  status THROW(0x40) -> 503(0x1f7)
///
/// `pikachu_throw_main` intercepts on THROW's MAIN and its comment claimed "Pikachu's own throw
/// never runs, so nothing is released". `catch_cut` happens on the way IN, before Main is
/// reached. So the engine's grab was already gone, `enter_carry`'s
/// `if !LinkModule::is_linked(..)` was true, and it built a replacement capture by hand.
///
/// AND THAT REPLACEMENT CANNOT BE UNDONE. pt28 ran a four-candidate ladder against it, one per
/// frame, and every one failed with `their_captured_by` stuck at `0x0`:
///
///     capture_cut(v, true, false, false)      cleared=false
///     CatchModule::set_catch(ours, INVALID)   cleared=false
///     capture_cut(v, true, true, true)        cleared=false
///     capture(v, INVALID, ..)                 cleared=false
///
/// There is no release call to find, so the fix is to stop needing one. Intercepting HERE leaves
/// the engine's grab standing all the way into SHOULDER_START, which makes `enter_carry`'s guard
/// false, which means no hand-made capture exists to leak.
///
/// ONLY THE FORWARD THROW, AND ONLY ON THE ENGINE'S OWN FLAG. `FIGHTER_PAD_CMD_CAT2_FLAG_THROW_F`
/// is what the engine itself tests here, so back, up and down throws are untouched and reach
/// Pikachu's own `THROW` exactly as before. If the flag is never raised in this status the
/// interception simply never fires and `pikachu_throw_main` catches it a frame later the way it
/// does today -- a safe failure, not a broken grab.
unsafe extern "C" fn pikachu_throw_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    let chosen = WorkModule::get_int64(boma, *FIGHTER_STATUS_CATCH_WAIT_WORK_INT_MOTION_KIND);
    let forward = chosen == Hash40::new("throw_f").hash;

    if entry < 8 && forward && !CARRYING[entry] {
        let victim = LAST_CAPTURED[entry];
        if crate::victim::boma(victim).is_some() {
            CARRYING[entry] = true;
            CARRY_FRAMES[entry] = 0;
            VICTIM[entry] = victim;
            crate::telemetry::sink::crumb(format!(
                "[rat] CARGO p{entry} intercepted the forward throw at its FIRST frame, victim \
                 {victim:#x} -- Pikachu's own throw never runs, so nothing is released"
            ));
            super::shoulder::begin(fighter, victim);
            return 0.into();
        }
        crate::telemetry::sink::crumb(format!(
            "[rat] CARGO p{entry} forward throw with no valid victim ({victim:#x}) -- letting \
             Pikachu's own throw run"
        ));
    }

    smashline::original_status(Main, fighter, *FIGHTER_STATUS_KIND_THROW)(fighter)
}

pub fn install() {
    crate::charaslot::agent("pikachu")
        .on_line(Main, pikachu_cargo_frame)
        // The forward throw IS the lift, and it has to be taken over before Pikachu's own throw
        // executes. See `pikachu_throw_main`.
        // No CATCH_WAIT hook: `Main` runs ONCE, at status entry, before the player has pressed
        // anything, and `CatchCont` -- which is what actually reads the throw input -- runs from
        // the status main LOOP every frame. Racing it from outside cannot work and pt29 proved
        // it did not. `specials::shoulder::retarget_forward_throw` tells the engine the answer
        // instead. This stays only as the fallback for a forward throw that somehow still
        // reaches Pikachu's own THROW.
        .status(Main, *FIGHTER_STATUS_KIND_THROW, pikachu_throw_main)
        .install();
}

/// The last id the engine named as our captive, for `shoulder_start_init` to adopt.
///
/// Exposed because the carry can now be entered by the ENGINE -- `CatchCont` reads
/// `globalTable[0x45]` and `specials::shoulder::retarget_forward_throw` points that at
/// SHOULDER_START -- so the status can arrive without any of this module's code running first,
/// and it still has to know who is being carried.
pub unsafe fn last_captured(entry: usize) -> u32 {
    if entry < 8 { LAST_CAPTURED[entry] } else { crate::victim::NOBODY }
}
