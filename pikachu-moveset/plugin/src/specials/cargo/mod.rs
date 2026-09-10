// Donkey Kong's cargo carry (f-throw): re-grab instead of launching, then four directional throws.
//
// WHAT IS AND IS NOT REPRODUCED, stated plainly because this is the one feature in the brief that
// cannot be ported whole.
//
// The victim side is engine-global: FIGHTER_STATUS_KIND_SHOULDERED_DONKEY, _START and _THROWN are
// in the common status list, and DK's game_throwf gets there with
// ATTACK_ABS(FIGHTER_ATTACK_ABSOLUTE_KIND_CATCH, ...) -- a catch-kind throw that RE-CATCHES the
// victim rather than launching them -- followed by a work flag on frame 16 that his C++ turns into
// SHOULDER_WAIT. The four cargo throws then read FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT,
// which is also global.
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

unsafe fn entry_of(boma: *mut BattleObjectModuleAccessor) -> usize {
    let e = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if e >= 8 { 0 } else { e }
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
        return;
    }

    if !CARRYING[entry] {
        return;
    }
    CARRY_FRAMES[entry] += 1;

    // Losing the victim for any reason -- they mashed out, we got hit -- ends the carry.
    if !CatchModule::is_catch(boma) {
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
            MotionModule::change_motion(boma, Hash40::new(m), 0.0, 1.0, false, 0.0, false, false);
            CARRYING[entry] = false;
            CARRY_FRAMES[entry] = 0;
            return;
        }
    }

    // Release valve. Without this a carry that never receives an input would hold the victim
    // indefinitely, which is a worse bug than the missing locomotion.
    if CARRY_FRAMES[entry] >= CARRY_TIMEOUT {
        MotionModule::change_motion(boma, Hash40::new("throw_f_f"), 0.0, 1.0, false, 0.0, false, false);
        CARRYING[entry] = false;
        CARRY_FRAMES[entry] = 0;
    }
}

pub fn install() {
    Agent::new("pikachu")
        .on_line(Main, pikachu_cargo_frame)
        .install();
}
