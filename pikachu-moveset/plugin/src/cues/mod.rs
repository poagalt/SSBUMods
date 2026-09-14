// The two impact cues the playtest asked for, and an honest note about where they come from.
//
// "make sure things like the screen effects for a little mac fsmash landing when super armor
// triggered and landing an alolan whip lariat while revenge is active are implemented".
//
// NEITHER IS A TRANSCRIPTION, AND THAT IS NOT A SHORTCUT -- THERE IS NOTHING TO TRANSCRIBE.
//
//   * Little Mac's armor is already ported and works: `damage!(MA_MSC_DAMAGE_DAMAGE_NO_REACTION,
//     DAMAGE_NO_REACTION_MODE_ALWAYS, 0)` on frame 8 of his f-smash, back to `_MODE_NORMAL` on 14,
//     copied verbatim from his script. His script contains NO cue for the absorb -- whatever
//     vanilla shows comes from the engine's own no-reaction path, which is not a call this port
//     can make.
//
//   * Incineroar's boosted hit has no script cue either. `lua2cpp_gaogaen.nro` reads
//     FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLAG_IS_ATTACK_HIT_REVENGE_CONTINUE at 0x710000a1f0 and
//     what it does with it is `FighterSpecializer_Gaogaen::revenge_off` -- that is the boost being
//     SPENT, which `speciallw` already models. Not a visual.
//
// So the effects below are chosen, not copied, and they are `sys_` ones under ADR 0002 -- the same
// standing as `sys_final_aura` substituting for `gaogaen_revenge_aura`. Every one is already used
// elsewhere in this moveset, so none of them is a guess about what exists.
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

/// Damage last seen, per entry, so an absorb can be detected as a rise.
static mut LAST_DAMAGE: [f32; 8] = [0.0; 8];
/// So a single absorb fires one cue rather than one per frame of hitlag.
static mut ARMOR_COOLDOWN: [i32; 8] = [0; 8];

/// Frames after an absorb cue during which another will not fire.
const COOLDOWN: i32 = 12;

unsafe fn entry_of(boma: *mut BattleObjectModuleAccessor) -> usize {
    let e = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if e >= 8 { 0 } else { e }
}

/// The nine common reaction statuses, `DAMAGE` through `DAMAGE_FALL`.
///
/// THIS IS THE WHOLE DETECTOR, AND IT IS WHY NO FRAME NUMBERS APPEAR IN THIS FILE. Armor absorbing
/// a hit IS "damage went up and the fighter did not enter a reaction status" -- that is the
/// definition, not a proxy for it. The alternative was to restate the armor window from the ACMD
/// here, and this project has a name for two copies of the same numbers: `tools/check-lists.sh`
/// exists because three hand-kept lists of the same 52 moves drifted.
///
/// It also means this works for any armored move the moveset ever gains, not just the f-smash, and
/// that it cannot disagree with the script it is watching.
///
/// `DamageModule::check_no_reaction` would be the direct question but it takes a `DamageInfo`
/// pointer, so it cannot be asked outside the damage path itself.
fn is_reaction_status(status: i32) -> bool {
    unsafe {
        status >= *FIGHTER_STATUS_KIND_DAMAGE && status <= *FIGHTER_STATUS_KIND_DAMAGE_FALL
    }
}

/// The armored-hit cue. One flash and one impact burst, at the point the hit was shrugged off.
unsafe fn armor_absorbed(fighter: &mut L2CFighterCommon, entry: usize, took: f32) {
    ARMOR_COOLDOWN[entry] = COOLDOWN;
    macros::EFFECT(
        &mut **fighter, Hash40::new("sys_attack_impact"), Hash40::new("top"),
        0, 8, 0, 0, 0, 0, 1.3, 0, 0, 0, 0, 0, 0, true,
    );
    macros::EFFECT(
        &mut **fighter, Hash40::new("sys_flash"), Hash40::new("top"),
        0, 8, 0, 0, 0, 0, 1.0, 0, 0, 0, 0, 0, 0, true,
    );
    crate::telemetry::probe::event(format!(
        "[rat] CUE p{entry} armor absorbed {took:.1}% without a reaction status; {}",
        crate::telemetry::probe::ctx(fighter.module_accessor)
    ));
}

/// A boosted attack of ours connected. Called from `speciallw` at the point the boost is spent,
/// which is exactly the native condition -- Incineroar calls `revenge_off` there too.
///
/// Scaled by the boost, because a 3.0 rate and a 1.1 rate should not look the same. `rate` is the
/// multiplier `speciallw` has banked, 1.0 to `RATE_MAX` = 3.0.
pub unsafe fn revenge_impact(fighter: &mut L2CFighterCommon, rate: f32) {
    let t = ((rate - 1.0) / 2.0).clamp(0.0, 1.0);
    macros::EFFECT(
        &mut **fighter, Hash40::new("sys_smash_flash"), Hash40::new("top"),
        0, 8, 0, 0, 0, 0, 1.0 + t, 0, 0, 0, 0, 0, 0, true,
    );
    macros::EFFECT(
        &mut **fighter, Hash40::new("sys_attack_speedline"), Hash40::new("top"),
        0, 8, 0, 0, 0, 0, 1.0 + t, 0, 0, 0, 0, 0, 0, true,
    );
}

unsafe extern "C" fn pikachu_cue_frame(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    let damage = DamageModule::damage(boma, 0);
    let took = damage - LAST_DAMAGE[entry];
    LAST_DAMAGE[entry] = damage;

    if ARMOR_COOLDOWN[entry] > 0 {
        ARMOR_COOLDOWN[entry] -= 1;
        return;
    }
    // A RISE WITH NO REACTION STATUS. Revenge's own stance is excluded because it arms
    // DAMAGE_NO_REACTION_MODE_ALWAYS on purpose for the whole counter window -- absorbing there is
    // the move working, and it already has its own aura and its own counter cue.
    if took > 0.01
        && !is_reaction_status(StatusModule::status_kind(boma))
        && !crate::specials::speciallw::absorb_armed(entry)
    {
        armor_absorbed(fighter, entry, took);
    }
}

pub fn install() {
    Agent::new("pikachu")
        .on_line(Main, pikachu_cue_frame)
        .install();
}
