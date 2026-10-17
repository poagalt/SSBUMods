//! Does the air-damage crash happen inside PIKACHU's status, or somebody else's?
//!
//! THE CRASH. Being hit in the air takes an `Instruction Abort` at PC=0 -- a call through a null
//! function pointer -- from `lua2cpp_common` `ftStatusUniqProcessDamageAir_init + 0x580`, reached
//! through `sub_damage_uniq_process_init` on `sys_line_status_system_control`.
//!
//! TWO PROBES BEFORE THIS ONE ANSWERED NOTHING, and both failures are worth recording because they
//! are the same mistake in different clothes -- instrumenting a path the thing does not take:
//!
//!   1. Breadcrumbs at the top of our six `on_line(Main, ..)` callbacks. The crashing function is a
//!      status `_init`, which runs BEFORE any Main line of that status, so they could not fire.
//!   2. A hook on `DamageModule::add_damage`. That is the LUA accessor; the engine applies damage
//!      natively and never goes through it. Zero lines, on a boot that crashed. Exactly the reach
//!      trap that cost two playtests on Little Mac's jab.
//!
//! THIS ONE IS ON THE FAULTING FUNCTION ITSELF. It registers `Init` on `FIGHTER_STATUS_KIND_
//! DAMAGE_AIR` and brackets the original: a line before, a line after. `original_status` is the
//! same mechanism `specials::cargo` already uses on `THROW`/`Main`, so the behaviour is unchanged.
//!
//! HOW TO READ THE RESULT, which is the whole point of bracketing:
//!
//!   "entered" and NO "returned"  -> the fault is inside the engine's own DAMAGE_AIR init, on
//!                                   PIKACHU. The port put something in his state that that
//!                                   function cannot survive, and the next step is which.
//!   neither line                 -> it is NOT Pikachu's status. We only register on the pikachu
//!                                   agent, so the faulting fighter is the opponent, and the
//!                                   question becomes what we do to everybody else.
//!   both lines, then a crash     -> the init survives and the fault is later in the same frame.
//!
//! `diag`, not `log`: `sink::log` queues for a flush every 120 frames and this crash kills the
//! game inside the frame -- the last telemetry on the card was an unrelated `WALK -> WAIT`.
use {
    smash::{lua2cpp::L2CFighterCommon, lib::{lua_const::*, L2CValue}},
    // GLOB, because the typed line markers (`Init`, `Main`, ..) are unit structs that satisfy
    // `StatusLineMarker`, not variants of the `StatusLine` enum. Importing `StatusLine::*`
    // compiles the names and then fails the trait bound -- same import block as every other
    // status module here, for the same reason.
    smashline::{*, Priority::*},
};

/// Set false to silence the probe without unpicking the registration.
pub const ENABLED: bool = true;

unsafe extern "C" fn damage_air_init_probe(fighter: &mut L2CFighterCommon) -> L2CValue {
    if ENABLED {
        crate::telemetry::sink::diag(
            "[rat] DAMAGEPROBE DAMAGE_AIR Init ENTERED on pikachu -- if no RETURNED line follows, \
             the fault is inside the engine's own init and it is Pikachu's status"
                .into(),
        );
    }
    let result = smashline::original_status(Init, fighter, *FIGHTER_STATUS_KIND_DAMAGE_AIR)(fighter);
    if ENABLED {
        crate::telemetry::sink::diag(
            "[rat] DAMAGEPROBE DAMAGE_AIR Init RETURNED -- the init survived".into(),
        );
    }
    result
}

pub fn install() {
    if !ENABLED {
        return;
    }
    crate::charaslot::agent("pikachu")
        .status(Init, *FIGHTER_STATUS_KIND_DAMAGE_AIR, damage_air_init_probe)
        .install();
}
