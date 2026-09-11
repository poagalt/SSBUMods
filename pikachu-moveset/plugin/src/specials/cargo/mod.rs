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
const CARRY_TIMEOUT: i32 = 120;

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

/// Ask the carried fighter to change status. Their SHOULDERED_DONKEY statuses are common engine
/// code, so this is the whole of the victim side -- being held on the shoulder, tracking the
/// carrier and being launched are all theirs to run.
/// Whether to push the victim into Donkey Kong's own SHOULDERED statuses.
///
/// OFF, AND THIS IS NOW THE LEADING SUSPECT FOR THE FREEZE.
///
/// Round 2's fixes -- trigger instead of held, and waiting for the engine's THROW status to end
/// before touching the motion -- did not change the crash at all. It is byte-identical: a null
/// dereference inside `MotionModule::change_motion`, reached through
/// `bind_hash_call_call_line_status` from a smashline line callback. And `librat_of_doom` appears
/// nowhere in the report: not in a frame, not in the module list.
///
/// So the change_motion that dies is probably not ours. The remaining thing this module does that
/// reaches outside Pikachu is exactly this: it asks the VICTIM to enter SHOULDERED_DONKEY_START
/// and then SHOULDERED_DONKEY. Those statuses are engine-common, which is why they were believed
/// safe -- but common only means every fighter HAS them, not that they work with a captor who is
/// not Donkey Kong. Their status code runs on the victim, through the same
/// `bind_hash_call_call_line_status` path in the backtrace, and being shouldered is defined
/// relative to a carrier it then reads. A carrier that is a Pikachu in a THROW status is not
/// something that code was written for.
///
/// With this off the victim simply stays CAUGHT -- DK's `throw_f` catches with an ATTACK_ABS of
/// CATCH kind rather than THROW kind, so they are held either way. What is lost is their pose:
/// they hang in the ordinary capture animation instead of sitting on the shoulder. That is a
/// visible deviation from the native move and it is the deliberate price of not freezing.
///
/// THE NEXT LOG DECIDES IT. Every frame of the carry now records what the victim's status
/// actually is, so "they stay captured" and "they fall out immediately" are distinguishable, and
/// if the freeze persists with this off then the cause is somewhere this module cannot reach and
/// the search moves to the engine's own throw handling.
const TELL_VICTIM_SHOULDERED: bool = false;

unsafe fn tell_victim(entry: usize, status: i32) {
    use crate::telemetry::probe;
    let id = VICTIM[entry];
    if !TELL_VICTIM_SHOULDERED {
        probe::event(format!(
            "[rat] CARGO p{entry} NOT requesting status {} on victim {id:#x} \
             (TELL_VICTIM_SHOULDERED is off -- suspected cause of the freeze)",
            probe::st(status)
        ));
        return;
    }
    let Some(vboma) = crate::victim::boma(id) else {
        probe::event(format!(
            "[rat] CARGO p{entry} victim id {id:#x} does not resolve to a live accessor, not \n             telling it {}",
            probe::st(status)
        ));
        return;
    };
    // FLUSHED BEFORE THE CALL, deliberately. If this is what kills the game, the last line in the
    // file names it -- which is the whole reason the previous round could not place the crash.
    probe::event(format!("[rat] CARGO p{entry} -> victim {id:#x} status {}", probe::st(status)));
    StatusModule::change_status_request_from_script(vboma, status, false);
    probe::event(format!("[rat] CARGO p{entry} victim status request returned"));
}

/// What the victim is actually doing, for the carry probe. Returns -1 when unreadable.
unsafe fn victim_status(entry: usize) -> i32 {
    match crate::victim::boma(VICTIM[entry]) {
        Some(vboma) => StatusModule::status_kind(vboma),
        None => -1,
    }
}

// Which cargo throw the stick asks for. Unlike Alolan Whip's follow-up mapping, this one is NOT
// interpreted: cargo throws map to the four directions exactly as ordinary throws do.
unsafe fn cargo_direction(boma: *mut BattleObjectModuleAccessor) -> Option<&'static str> {
    let x = ControlModule::get_stick_x(boma);
    let y = ControlModule::get_stick_y(boma);
    let lr = PostureModule::lr(boma);
    if y > 0.5 {
        Some("throw_f_hi")
    } else if y < -0.5 {
        Some("throw_f_lw")
    } else if x * lr > 0.5 {
        Some("throw_f_f")
    } else if x * lr < -0.5 {
        Some("throw_f_b")
    } else {
        None
    }
}

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

    // The cargo grab is DK's throw_f. Once its motion finishes, the victim is still caught
    // (ATTACK_ABS of CATCH kind, not THROW kind), so this is the moment the carry begins.
    if !CARRYING[entry] && motion == smash::hash40("throw_f") && MotionModule::is_end(boma) {
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
        CARRYING[entry] = false;
        return;
        // DK's frame-16 SHOULDER_WAIT flag is what his C++ turns into this. Requesting it here is
        // the same handoff, minus the C++.
        tell_victim(entry, *FIGHTER_STATUS_KIND_SHOULDERED_DONKEY_START);
        return;
    }

    if !CARRYING[entry] {
        return;
    }
    CARRY_FRAMES[entry] += 1;

    // THE FREEZE WAS HERE: WE TOUCHED THE MOTION WHILE THE ENGINE STILL OWNED IT.
    //
    // The crash report is a Data Abort on a null pointer inside
    // `MotionModule::change_motion`, reached from a smashline line callback -- ours. The
    // telemetry's last line is `CATCH_PULL -> THROW`, so Pikachu was still inside the engine's
    // own THROW status, which is running throw_f and owns the fighter's motion for its duration.
    // Changing motion out from under it is what died.
    //
    // Two things let that happen, and both are fixed:
    //
    //   1. `check_button_on` is HELD, not pressed. The grab button was still down from the grab
    //      itself, so with any stick direction the throw committed on the FIRST carry frame --
    //      inside THROW. The whip module already uses `check_button_trigger` for exactly this
    //      reason; this module never got the same treatment.
    //   2. Nothing checked the status before changing motion.
    //
    // So the carry now waits for the engine to leave THROW before it touches anything. The
    // handoff to the victim still happens immediately -- that is a status request on THEM, not a
    // motion change on us, and it is what puts Mario on the shoulder.
    if status == *FIGHTER_STATUS_KIND_THROW || status == *FIGHTER_STATUS_KIND_CATCH_ATTACK {
        if CARRY_FRAMES[entry] == 2 {
            tell_victim(entry, *FIGHTER_STATUS_KIND_SHOULDERED_DONKEY);
            probe::event(format!(
                "[rat] CARGO p{entry} shoulder handoff sent, waiting for THROW to end; {}",
                probe::ctx(boma)
            ));
        }
        return;
    }

    // SHOULDERED_DONKEY_START is a transition; the hold itself is SHOULDERED_DONKEY. Requesting it
    // once, a frame later, lets their own status code settle onto the shoulder.
    // Belt to the braces above: if the engine left THROW before frame 2, the handoff still has to
    // go out. Requesting the same status twice is harmless -- it is already their current one.
    if CARRY_FRAMES[entry] <= 3 {
        tell_victim(entry, *FIGHTER_STATUS_KIND_SHOULDERED_DONKEY);
    }

    // THE CARRY POSE. Pikachu is in his own movement status, so his speeds and jump are his; only
    // the motion is DK's. change_motion is guarded on the motion actually differing, because
    // calling it every frame would restart the animation every frame.
    // EVERY FRAME OF THE CARRY, because the previous two rounds each ended with a freeze whose
    // surrounding frames were not in the log. Buffered rather than flushed -- the `event` lines
    // either side anchor it, and a carry is at most 120 frames.
    probe::frame(format!(
        "[rat] CARGO p{entry} carry f={} victim={:#x} victim_st={}; {}",
        CARRY_FRAMES[entry],
        VICTIM[entry],
        probe::st(victim_status(entry)),
        probe::ctx(boma)
    ));

    if let Some(pose) = carry_pose(boma) {
        if MotionModule::motion_kind(boma) != smash::hash40(pose) {
            probe::change_motion(boma, pose, "CARGO pose");
        }
    }
    suppress_carry_actions(boma);

    // Losing the victim for any reason -- they mashed out, we got hit -- ends the carry.
    // is_catch is not consulted after the handoff: the victim is in a shouldered status now, not
    // an ordinary capture, so it reads false and used to end the carry immediately.
    if CARRY_FRAMES[entry] > 2 && !victim_still_held(entry) {
        probe::event(format!(
            "[rat] CARGO p{entry} victim lost after {}f, carry ends; {}",
            CARRY_FRAMES[entry],
            probe::ctx(boma)
        ));
        CARRYING[entry] = false;
        CARRY_FRAMES[entry] = 0;
        return;
    }

    // A direction plus the attack or special button commits to a throw. Changing motion is enough
    // to run that motion's game script, so no additional status kinds are needed -- the same
    // reason Revenge and Alolan Whip get by with one status each.
    //
    // TRIGGER, NOT ON. `check_button_on` is true while the button is HELD, and the grab button is
    // still down at the moment the grab lands -- so the throw committed on the first carry frame,
    // before Mario had finished being lifted, and did it from inside the engine's THROW status.
    // That is what froze the game. ../specials uses check_button_trigger for the whip's
    // follow-ups and always did; this module simply never got the same treatment.
    let pressed = ControlModule::check_button_trigger(boma, *CONTROL_PAD_BUTTON_ATTACK)
        || ControlModule::check_button_trigger(boma, *CONTROL_PAD_BUTTON_SPECIAL);
    if pressed {
        if let Some(m) = cargo_direction(boma) {
            throw_cargo(boma, entry, m);
            return;
        }
    }

    // Release valve. Without this a carry that never receives an input would hold the victim
    // indefinitely, which is a worse bug than the missing locomotion.
    if CARRY_FRAMES[entry] >= CARRY_TIMEOUT {
        throw_cargo(boma, entry, "throw_f_f");
    }
}

/// Commit to one of the four cargo throws. The motion's own game script carries the
/// ATTACK_ABS(THROW) that launches the victim; SHOULDERED_DONKEY_THROWN is what puts them in the
/// matching thrown_f_donkey_* animation on the way out.
unsafe fn throw_cargo(boma: *mut BattleObjectModuleAccessor, entry: usize, motion: &str) {
    use crate::telemetry::probe;
    probe::event(format!(
        "[rat] CARGO p{entry} throw '{motion}' after {}f held; {}",
        CARRY_FRAMES[entry],
        probe::ctx(boma)
    ));
    probe::change_motion(boma, motion, "CARGO throw");
    tell_victim(entry, *FIGHTER_STATUS_KIND_SHOULDERED_DONKEY_THROWN);
    CARRYING[entry] = false;
    CARRY_FRAMES[entry] = 0;
    VICTIM[entry] = *BATTLE_OBJECT_ID_INVALID as u32;
}

/// Which of DK's carry poses matches the movement status Pikachu is currently in.
///
/// Returns None for a status the carry does not cover, which leaves whatever motion is playing
/// alone rather than forcing a pose onto a state this has not thought about -- being knocked down
/// while carrying, for instance, should look like being knocked down.
///
/// The three walk speeds map off Pikachu's own walk motion rather than off a speed threshold,
/// because the engine has already picked slow/middle/fast from his stick input and his
/// walk_speed_max. Reading its choice keeps the pose in step with the locomotion for free.
unsafe fn carry_pose(boma: *mut BattleObjectModuleAccessor) -> Option<&'static str> {
    let status = StatusModule::status_kind(boma);
    if status == *FIGHTER_STATUS_KIND_WAIT {
        return Some("shoulder_wait");
    }
    if status == *FIGHTER_STATUS_KIND_TURN {
        return Some("shoulder_turn");
    }
    if status == *FIGHTER_STATUS_KIND_JUMP_SQUAT {
        return Some("shoulder_jump_squat");
    }
    if status == *FIGHTER_STATUS_KIND_JUMP {
        return Some("shoulder_jump");
    }
    if status == *FIGHTER_STATUS_KIND_FALL {
        return Some("shoulder_fall");
    }
    if status == *FIGHTER_STATUS_KIND_LANDING {
        return Some("shoulder_landing");
    }
    // WALK and DASH both get a walk pose; DK has no carried dash, and his fast walk is the closest
    // thing he owns.
    if status == *FIGHTER_STATUS_KIND_WALK || status == *FIGHTER_STATUS_KIND_DASH {
        let m = MotionModule::motion_kind(boma);
        if m == smash::hash40("walk_slow") {
            return Some("shoulder_walk_slow");
        }
        if m == smash::hash40("walk_fast") || status == *FIGHTER_STATUS_KIND_DASH {
            return Some("shoulder_walk_fast");
        }
        return Some("shoulder_walk_middle");
    }
    None
}

/// Takes away the actions a carrier should not have.
///
/// Without this, Pikachu is in an ordinary WALK or WAIT and can attack, shield or throw out a
/// special while holding someone -- and each of those drops the victim in a state nothing is
/// tracking. DK simply does not have those options while carrying. Disabling the transition groups
/// is the engine's own mechanism for exactly that, and it lasts one frame, so it is reapplied every
/// frame the carry is live and never has to be undone when the carry ends.
unsafe fn suppress_carry_actions(boma: *mut BattleObjectModuleAccessor) {
    for group in [
        *FIGHTER_STATUS_TRANSITION_GROUP_CHK_GROUND_ATTACK,
        *FIGHTER_STATUS_TRANSITION_GROUP_CHK_GROUND_SPECIAL,
        *FIGHTER_STATUS_TRANSITION_GROUP_CHK_GROUND_GUARD,
        *FIGHTER_STATUS_TRANSITION_GROUP_CHK_AIR_ATTACK,
        *FIGHTER_STATUS_TRANSITION_GROUP_CHK_AIR_SPECIAL,
    ] {
        WorkModule::unable_transition_term_group(boma, group);
    }
    // NO DASHING OR RUNNING WHILE CARRYING, which is the restriction playtest noticed was
    // missing: "doesn't have the less movement speed limit that cargo throw places on dk".
    //
    // Worth being precise about what that limit is, because the param says the opposite of what
    // it sounds like. `donkey param_lifting_walk` multiplies walk speed and acceleration by 1.1 --
    // carrying makes him WALK slightly faster, not slower. What he loses is the fast options: DK's
    // carry is a walk-only mode, with no dash and no run, and that is what makes it feel slow.
    //
    // Suppressing the transitions is exactly how the engine expresses that, and unlike the walk
    // multipliers it does not compound across frames -- these are re-asserted every frame by
    // design, which is why they are safe here and `mul_x_speed_max` is not.
    // Dash and run are individual TERMS, not a group -- the only ground groups the engine has are
    // ATTACK, CATCH, ESCAPE, GUARD, ITEM, JUMP and SPECIAL. So they are forbidden one by one.
    for term in [
        *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_DASH,
        *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_TURN_DASH,
        *FIGHTER_STATUS_TRANSITION_TERM_ID_DASH_TO_RUN,
        *FIGHTER_STATUS_TRANSITION_TERM_ID_RUN,
        *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_TURN_RUN,
    ] {
        WorkModule::unable_transition_term(boma, term);
    }
}

/// Whether the carried fighter is still in one of the shouldered statuses.
unsafe fn victim_still_held(entry: usize) -> bool {
    let Some(vboma) = crate::victim::boma(VICTIM[entry]) else { return false };
    let s = StatusModule::status_kind(vboma);
    // WITH TELL_VICTIM_SHOULDERED OFF THEY NEVER REACH A SHOULDERED STATUS, so requiring one
    // ended the carry on its third frame every time. An ordinary capture counts as held too --
    // which is what they are in now, and what DK's throw_f leaves them in regardless.
    s == *FIGHTER_STATUS_KIND_SHOULDERED_DONKEY
        || s == *FIGHTER_STATUS_KIND_SHOULDERED_DONKEY_START
        || s == *FIGHTER_STATUS_KIND_CAPTURE_PULLED
        || s == *FIGHTER_STATUS_KIND_CAPTURE_WAIT
        || s == *FIGHTER_STATUS_KIND_CAPTURE_CUT
}

pub fn install() {
    Agent::new("pikachu")
        .on_line(Main, pikachu_cargo_frame)
        .install();
}
