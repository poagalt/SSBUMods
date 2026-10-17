// Makes Pikachu's own replaced specials UNREACHABLE, rather than merely unused.
//
// WHY THIS IS NOT PARANOIA. tools/motion-patch/src/bin/variants.rs enumerates every motion Pikachu
// has that a move's source fighter does not, and six of the eight it finds are his own specials:
//
//   special_s, special_s_hold, special_s_weak   Skull Bash -- release, charge, weak release
//   special_lw_hit                              Thunder's hit reaction
//   special_hi_start, special_hi_end            Quick Attack
//
// Incineroar and Game & Watch have no counterparts, so nothing overrides them and the animations
// are still sitting there. They are only unreachable because this moveset never REQUESTS the
// statuses that play them -- which is an argument about code that does not exist, and this moveset
// has already been wrong about that twice. The up-B remap ran Pikachu's Quick Attack Init and Exec
// for a whole version because only Pre/Main/End were registered, and playtest reported "charging
// animations are still pikachu's" at a point when the charge status was believed unreachable.
//
// A status you have not registered is not yours. So each of these gets a handler that leaves
// immediately, and the guarantee stops depending on nobody ever adding a transition term.
//
// THE FINAL SMASH IS DELIBERATELY EXCLUDED. Pikachu's 0x1E8..0x1EF are Volt Tackle, which this
// moveset does not replace and must keep working.
//
// Cost of being wrong in this direction: if one of these statuses turns out to be needed, the
// symptom is Pikachu dropping to WAIT instead of doing something. That is visible and immediate,
// unlike a Skull Bash animation appearing inside a command grab.
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

/// Pikachu's own extra status kinds for the specials this moveset replaces. Values from
/// skyline-smash's constant table; his table runs 0x1E1..0x1EF with NUM = 0x1F0.
const RETIRED: &[(i32, &str)] = &[
    (0x1E1, "SPECIAL_HI_WARP    Quick Attack, second leg"),
    (0x1E2, "SPECIAL_HI_END     Quick Attack, recovery"),
    (0x1E3, "SPECIAL_S_HOLD     Skull Bash, charging"),
    (0x1E4, "SPECIAL_S_WEAK     Skull Bash, weak release"),
    (0x1E5, "SPECIAL_S_ATTACK   Skull Bash, release"),
    (0x1E6, "SPECIAL_S_END      Skull Bash, recovery"),
    (0x1E7, "SPECIAL_LW_HIT     Thunder, hit reaction"),
];

// Init settings deliberately minimal: this status exists only to be left. Using the situation the
// fighter is already in avoids yanking him into the air or onto the ground on the way out.
unsafe extern "C" fn retired_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    StatusModule::init_settings(
        fighter.module_accessor,
        smash::app::SituationKind(*SITUATION_KIND_NONE),
        *FIGHTER_KINETIC_TYPE_UNIQ,
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
        0,
        0,
        0,
        0
    );
    0.into()
}

unsafe extern "C" fn retired_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    // Loud, because reaching one of these means something requested a status this moveset thought
    // it had retired -- and that is exactly the class of bug the lockout exists to surface rather
    // than hide. Cheap: it can only fire on a path that should never run.
    println!(
        "[rat] LOCKOUT: retired Pikachu special status {} was requested -- bouncing to wait/fall",
        StatusModule::status_kind(boma)
    );
    if StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND {
        fighter.change_status(FIGHTER_STATUS_KIND_WAIT.into(), false.into());
    } else {
        fighter.change_status(FIGHTER_STATUS_KIND_FALL.into(), false.into());
    }
    1.into()
}

unsafe extern "C" fn retired_end(fighter: &mut L2CFighterCommon) -> L2CValue {
    let _ = fighter;
    0.into()
}

pub fn install() {
    let mut agent = crate::charaslot::agent("pikachu");
    for (kind, _what) in RETIRED {
        agent
            .status(Pre, *kind, retired_pre)
            .status(Main, *kind, retired_main)
            .status(End, *kind, retired_end);
    }
    agent.install();
}
