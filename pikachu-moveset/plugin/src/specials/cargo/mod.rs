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
// SO: the carry here is STATIONARY. You get the cargo grab and all four directional throws,
// including the cargo up-throw, but you cannot walk or jump while holding. That is a real
// deviation from the brief and is called out rather than papered over.
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

static mut CARRYING: [bool; 8] = [false; 8];
static mut CARRY_FRAMES: [i32; 8] = [0; 8];
/// The carried fighter. Cached because `CatchModule::capture_object_id` stops answering once they
/// move into a shouldered status, and the throw still has to address them.
static mut VICTIM: [u32; 8] = [0; 8];

unsafe fn entry_of(boma: *mut BattleObjectModuleAccessor) -> usize {
    let e = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if e >= 8 { 0 } else { e }
}

/// Ask the carried fighter to change status. Their SHOULDERED_DONKEY statuses are common engine
/// code, so this is the whole of the victim side -- being held on the shoulder, tracking the
/// carrier and being launched are all theirs to run.
unsafe fn tell_victim(entry: usize, status: i32) {
    let id = VICTIM[entry];
    if id == *BATTLE_OBJECT_ID_INVALID as u32 || sv_battle_object::is_null(id) {
        return;
    }
    let vboma = sv_battle_object::module_accessor(id);
    if vboma.is_null() {
        return;
    }
    StatusModule::change_status_request_from_script(vboma, status, false);
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
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    let motion = MotionModule::motion_kind(boma);

    // The cargo grab is DK's throw_f. Once its motion finishes, the victim is still caught
    // (ATTACK_ABS of CATCH kind, not THROW kind), so this is the moment the carry begins.
    if !CARRYING[entry] && motion == smash::hash40("throw_f") && MotionModule::is_end(boma) {
        CARRYING[entry] = true;
        CARRY_FRAMES[entry] = 0;
        VICTIM[entry] = CatchModule::capture_object_id(boma, false) as u32;
        // DK's frame-16 SHOULDER_WAIT flag is what his C++ turns into this. Requesting it here is
        // the same handoff, minus the C++.
        tell_victim(entry, *FIGHTER_STATUS_KIND_SHOULDERED_DONKEY_START);
        return;
    }

    if !CARRYING[entry] {
        return;
    }
    CARRY_FRAMES[entry] += 1;

    // SHOULDERED_DONKEY_START is a transition; the hold itself is SHOULDERED_DONKEY. Requesting it
    // once, a frame later, lets their own status code settle onto the shoulder.
    if CARRY_FRAMES[entry] == 2 {
        tell_victim(entry, *FIGHTER_STATUS_KIND_SHOULDERED_DONKEY);
    }

    // Losing the victim for any reason -- they mashed out, we got hit -- ends the carry.
    // is_catch is not consulted after the handoff: the victim is in a shouldered status now, not
    // an ordinary capture, so it reads false and used to end the carry immediately.
    if CARRY_FRAMES[entry] > 2 && !victim_still_held(entry) {
        CARRYING[entry] = false;
        CARRY_FRAMES[entry] = 0;
        return;
    }

    // A direction plus the attack or special button commits to a throw. Changing motion is enough
    // to run that motion's game script, so no additional status kinds are needed -- the same
    // reason Revenge and Alolan Whip get by with one status each.
    let pressed = ControlModule::check_button_on(boma, *CONTROL_PAD_BUTTON_ATTACK)
        || ControlModule::check_button_on(boma, *CONTROL_PAD_BUTTON_SPECIAL);
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
    MotionModule::change_motion(boma, Hash40::new(motion), 0.0, 1.0, false, 0.0, false, false);
    tell_victim(entry, *FIGHTER_STATUS_KIND_SHOULDERED_DONKEY_THROWN);
    CARRYING[entry] = false;
    CARRY_FRAMES[entry] = 0;
    VICTIM[entry] = *BATTLE_OBJECT_ID_INVALID as u32;
}

/// Whether the carried fighter is still in one of the shouldered statuses.
unsafe fn victim_still_held(entry: usize) -> bool {
    let id = VICTIM[entry];
    if id == *BATTLE_OBJECT_ID_INVALID as u32 || sv_battle_object::is_null(id) {
        return false;
    }
    let vboma = sv_battle_object::module_accessor(id);
    if vboma.is_null() {
        return false;
    }
    let s = StatusModule::status_kind(vboma);
    s == *FIGHTER_STATUS_KIND_SHOULDERED_DONKEY
        || s == *FIGHTER_STATUS_KIND_SHOULDERED_DONKEY_START
}

pub fn install() {
    Agent::new("pikachu")
        .on_line(Main, pikachu_cargo_frame)
        .install();
}
