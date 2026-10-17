// The armored-absorb cue, and an honest account of the screen effect that was asked for.
//
// "make sure things like the screen effects for a little mac fsmash landing when super armor
// triggered and landing an alolan whip lariat while revenge is active are implemented", then:
// "the visual that happens for mac super armor and incin revenge lariat is a zoomed in blue
// screen, like the red kill screen but blue".
//
// THAT IS THE GAME'S FINISH CAMERA, AND IT IS NOT AN EFFECT. The investigation, so the next round
// does not repeat it:
//
//   * `EffectModule::req_screen` is the FINAL SMASH background system. All 597 calls across the
//     dumped scripts request a `bg_*_final` name; there is nothing else in it.
//   * `FILL_SCREEN_MODEL_COLOR` / `CANCEL_FILL_SCREEN` are the Final Smash DESATURATE. Every one
//     of the 160 calls passes `EFFECT_SCREEN_PRIO_FINAL` and a near-grey colour.
//   * `FighterCutInManager` + `FighterUtil::request_critical_hit_cut_in_force` are the Kazuya and
//     Joker style cinematic cut-ins, keyed on a `CutInType` and a `CutInData`.
//   * No `sys_*` effect for armor exists. Nothing matching "armor" appears in either hash
//     dictionary or in the 47,858-name hash40 table.
//
//   * What DOES exist is `sv_animcmd::CHECK_FINISH_CAMERA`, `CHECK_FINISH_CAMERA_IF_NOT_HP_MODE`,
//     `AttackModule::set_no_finish_camera` and eleven `FINISH_CAMERA` work ids including
//     `FIGHTER_INSTANCE_WORK_ID_FLAG_FINISH_CAMERA_DOLLY_STAGE`, which pairs with
//     `FighterUtil::request_critical_hit_cut_in_dolly_stage`. A dolly zoom on the stage: the
//     zoomed screen. `set_no_finish_camera` means the engine offers it BY DEFAULT and a fighter
//     opts out, which is why Little Mac's f-smash script contains no camera call and still gets
//     the effect in vanilla.
//
// SO THE LARIAT'S HALF WAS ALREADY RIGHT and the shoulder's was not. Both of Incineroar's side-B
// finishers ask for the camera; the lariat's `CHECK_FINISH_CAMERA` survived into
// `specials_acmd/mod.rs` and the shoulder's `CHECK_FINISH_CAMERA_IF_NOT_HP_MODE` did not, because
// smash-script does not export it and `deforeign` deletes what it cannot resolve. `src/acmd_ext`
// now supplies it. That is a transcription fix in the generated script, not something this file
// can do.
//
// WHICH LEAVES THIS FILE WITH ONE JOB. There is no cue to transcribe for the armored absorb --
// Little Mac's f-smash script has `damage!(MA_MSC_DAMAGE_DAMAGE_NO_REACTION,
// DAMAGE_NO_REACTION_MODE_ALWAYS, 0)` on frame 8 and `_MODE_NORMAL` on 14, and nothing else. The
// four `sys_` effects that used to be here were CHOSEN, not copied, and the playtest says they are
// not the visual; a second guess is worth less than the probe line, so they are gone and the
// detector stays. If the finish camera turns out not to cover the armor case either, the thing to
// find is the engine path, not another effect.
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

/// The armored-hit cue. NO EFFECT -- the probe line is the whole of it, deliberately.
///
/// This is the one place a guess would be invisible: an absorb is a non-event on screen, so a
/// wrong effect here reads as "the mod does something odd when Mac's f-smash is hit" rather than
/// as a missing feature. Keeping the detector means the moment is identified and logged, so the
/// next playtest can say whether vanilla's finish camera already covers it.
unsafe fn armor_absorbed(fighter: &mut L2CFighterCommon, entry: usize, took: f32) {
    ARMOR_COOLDOWN[entry] = COOLDOWN;
    crate::telemetry::probe::event(format!(
        "[rat] CUE p{entry} armor absorbed {took:.1}% without a reaction status; {}",
        crate::telemetry::probe::ctx(fighter.module_accessor)
    ));
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
    crate::charaslot::agent("pikachu")
        .on_line(Main, pikachu_cue_frame)
        .install();
}
