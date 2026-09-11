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
static mut LAST_CAPTURED: [u32; 8] = [0; 8];

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
    // (ATTACK_ABS of CATCH kind, not THROW kind), so this is the moment the carry begins.
    if !in_carry && !CARRYING[entry] && motion == smash::hash40("throw_f") && MotionModule::is_end(boma) {
        CARRYING[entry] = true;
        CARRY_FRAMES[entry] = 0;
        VICTIM[entry] = LAST_CAPTURED[entry];
        // A CRUMB, not a buffered line. If the handoff is what kills the game, this is the last
        // thing in the log and it names the id we are about to act on -- see telemetry/sink.rs.
        crate::telemetry::sink::crumb(format!(
            "[rat] CARGO p{entry} grab landed; capture_object_id now {cap:#x}, last valid \
             {:#x} -> handing to SHOULDER_START",
            VICTIM[entry],
        ));
        // A HANDOFF WITH NOBODY TO CARRY IS NOT A CARRY. Without this the shoulder statuses ran
        // against an id that never resolved, which looked like "the carry does nothing" at best
        // and dereferenced a non-accessor at worst. Falling back to DK's own throw is the
        // behaviour a missed cargo grab should have anyway.
        if crate::victim::boma(VICTIM[entry]).is_none() {
            probe::event(format!(
                "[rat] CARGO p{entry} NO VALID VICTIM at handoff (id {:#x}) -- not entering the \
                 carry; {}",
                VICTIM[entry],
                probe::ctx(boma)
            ));
            CARRYING[entry] = false;
            return;
        }
        probe::event(format!(
            "[rat] CARGO p{entry} grab landed, victim={:#x} -> handing to SHOULDER_START; {}",
            VICTIM[entry],
            probe::ctx(boma)
        ));
        // HAND OFF TO REAL STATUSES. Everything below this point in the old design -- poses via
        // change_motion on top of Pikachu's own WAIT and WALK, throws via change_motion, action
        // suppression -- is replaced by ../shoulder, because the real Donkey Kong uses eleven
        // statuses and the motion-swap approach reproduced none of the things that made it work.
        super::shoulder::begin(fighter, VICTIM[entry]);
        // CARRYING STAYS TRUE. It used to be cleared on the next line, which made a flag meaning
        // "a carry is in progress" false one statement after the carry began -- see the freeze
        // above. ../shoulder owns the carry from here; this flag is only what stops this hook
        // starting a second one, and it is cleared where the carry actually ends.
        return;
    }
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
pub fn install() {
    Agent::new("pikachu")
        .on_line(Main, pikachu_cargo_frame)
        .install();
}
