// Incineroar's Revenge (down-B): a counter that stores a damage and knockback boost.
//
// WHY THIS IS REIMPLEMENTED RATHER THAN PORTED
// SSBU counters are not in the ACMD dumps. The scripts only raise a status work flag -- Marth's
// `FLAG_SHIELD` on frames 6-28, Incineroar's `FLAG_STANCE_START`/`STANCE_END` on 8-28 -- and the
// "if hit while that flag is set, counter" logic lives in the fighter's C++ status code. smashline
// has no cross-fighter status sharing either: its share hooks cover the four ACMD categories
// (Game, Effect, Expression, Sound) and nothing else. So the window timings come from the ACMD and
// the detection is built from what a plugin can actually reach.
//
// THE TUNING IS THE GAME'S OWN, not invented. fighter/gaogaen/param/vl.prc, param_special_lw[0]:
//   special_lw_revenge_rate_base_add       = 0.5    flat boost per successful counter
//   special_lw_revenge_rate_attack_mul_add = 0.075  added per point of damage countered
//   special_lw_revenge_rate_max            = 3.0    cap -- this is the "stores x3" in the brief
//   special_lw_revenge_time                = 60     seconds an UNUSED boost survives
//   special_lw_hit_damage_mul              = 0.4    retaliation damage scales with what it ate
//   special_lw_revenge_cancel_percent      = 36     damage taken that DESTROYS a stored boost
// so rate = min(1 + 0.5 + 0.075 * countered_damage, 3.0), which for a 10% hit gives 2.25x.
//
// AND THE RULES ARE THE GAME'S OWN TOO, checked against SmashWiki's Revenge and Incineroar
// down-special pages rather than guessed:
//
//   * the boost is CONSUMED when Incineroar lands an attack -- it is a next-hit buff, not a
//     60-second aura. Playtest caught the difference: "revenge appears to have permanently stored
//     the extra damage rather than using it up on the next move". 60 seconds is only the ceiling
//     on an unused boost.
//   * counters STACK. A second counter with a boost already up adds to it rather than replacing
//     it, still capped at 3.0.
//   * cancel_percent is a LOSS condition on the STORED boost -- take 36% while holding one and it
//     is gone. It was previously read as a per-hit ceiling on whether the counter fires at all,
//     which is a different rule, and it also left the window stuck open when a big hit landed.
//   * being thrown loses the boost.
//   * the counterattack itself does NOT consume the boost. It belongs to the Revenge move; the
//     buff is for the attack after it.
//
// The one documented rule not reproduced: SmashWiki says the boost "decreases if Incineroar uses
// attacks that miss". Whiff detection is not something a plugin frame hook can do with any
// confidence, and getting it wrong would silently drain a boost that should still be there.
//
// COUNTER WINDOW TIMING IS EXACT, and worth writing down because it looks wrong. The stance ACMD
// opens the window on script frame 8 and closes it on 28, while Ultimate Frame Data lists the
// counter as active on frames 3-27. Both are right: FT_MOTION_RATE(r) is GAME FRAMES PER MOTION
// FRAME -- see smash_script's FT_MOTION_RATE_RANGE, which computes r = game_frames / motion_span
// -- so the stance's FT_MOTION_RATE(0.25) runs motion frames 1..9 four times faster than real
// time. Script frame 8 arrives on real frame ~3; the 1.5 and 1.0 stretches then put script frame
// 28 at real frame ~28. The window really is ~25 real frames at the very start of a much longer
// animation. Playtest read that as "I can only get it to trigger at the very start of the move",
// which is Incineroar's actual behaviour and is deliberately left alone.
//
// STATE IS PER-ENTRY STATICS, NOT WORK VARIABLES. Incineroar's own work ids cannot be reused:
// FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLAG_IS_ATTACK_HIT_REVENGE is 0x200000E3, which on Pikachu is
// FLAG_MTRANS_SMPL_MOTION_END. That class of collision is exactly what tools/reach deforeign
// exists to strip, and it is the same reason the ported d-air was writing Pikachu's FLAG_PAUSE.
// The George mod uses per-entry statics for the same reason.
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

// Read out of fighter/gaogaen/param/vl.prc with tools/prc-tool prcdump.
const RATE_BASE_ADD: f32 = 0.5;
const RATE_ATTACK_MUL_ADD: f32 = 0.075;
const RATE_MAX: f32 = 3.0;
const HIT_DAMAGE_MUL: f32 = 0.4;
/// Cumulative damage taken that destroys a stored boost.
const CANCEL_PERCENT: f32 = 36.0;
// `special_lw_revenge_time` is 60 and the label set includes `revenge_second`, so it is read as
// seconds. This is the ceiling on an UNUSED boost; landing an attack ends it far sooner.
const REVENGE_TIME_FRAMES: i32 = 60 * 60;
/// Real frames the counter stays live once opened. Belt to the ACMD's braces: if the stance is
/// interrupted before its frame-28 close_window, the window used to stay open indefinitely and a
/// hit long afterwards would still counter.
const WINDOW_FRAMES: i32 = 26;

// WHAT THIS PORT DOES NOT REPRODUCE, enumerated because tools/check-srcparams.sh asks and because
// "we did not know" and "we decided not to" should not look the same in six months.
//
// NOT MODELLED: special_lw_start_x_mul special_air_lw_speed_y_mul
//   Incineroar keeps 60% of his horizontal speed and 70% of his vertical when Revenge starts.
//   This port hard-stops instead -- FIGHTER_KINETIC_TYPE_GROUND_STOP / AIR_STOP in _main -- so a
//   running or rising Revenge plants where his drifts. NONE of his four Revenge ACMDs contains a
//   speed call, so both are applied by his C++ status and the names are all there is to go on:
//   "multiply the entering speed" is the obvious reading and "the start motion's own x energy" is
//   a real alternative. Implementing the wrong one turns a stop into a slide, which is worse than
//   the stop. THE PLAYTEST SETTLES IT: telemetry logs `moved x` and `peak` per motion, so run into
//   a grounded Revenge and rise into an air one, then do the same with Incineroar and compare.
//
// NOT MODELLED: special_lw_hit_stop_mul special_air_lw_hit_accel_y
//   1.5x hitlag on the counter hit, and 0.02 downward acceleration during the airborne one.
//   Both are status-side, both are small, and neither changes whether the move works.
//
// NOT MODELLED: special_lw_revenge_shield_damage_mul special_lw_revenge_rate_correct_max
// NOT MODELLED: special_lw_revenge_rate_max_team
//   Revenge also builds from shield damage at 0.2x, the stored rate has a separate correction
//   ceiling, and team battles cap it at 1.5 instead of 3.0. The shield case is real behaviour and
//   worth adding; the team cap only matters in a mode this mod is not aimed at.
//
// NOT MODELLED: special_lw_revenge_emmisive_scale_min special_lw_revenge_emmisive_scale_max
// NOT MODELLED: special_lw_revenge_pitch_min special_lw_revenge_pitch_max
// NOT MODELLED: special_lw_revenge_pitch_max_rate
//   The glow intensity and voice pitch scale with the stored rate. The glow is approximated with
//   sys_final_aura and an orange flash (ADR 0002 forbids Incineroar's own effects), and the pitch
//   would need his voice, which is not shipped.
//
// NOT MODELLED: critical_frame critical_slow_rate critical_zoom_rate critical_zoom_in_frame
// NOT MODELLED: critical_camera_offset_x critical_camera_offset_y critical_1_on_1
//   The KO cinematic on a Revenge-boosted finish. Pure presentation, and it belongs with the rest
//   of the visual layer that is deferred by design.

// The counter window is opened and closed by the hand-written stance ACMD in ../acmd_speciallw,
// at the SCRIPT frames Incineroar uses (8 and 28).
//
// It used to be derived from MotionModule::frame in the status loop, which was wrong twice over.
// First, script frames and motion frames are not the same thing: the stance runs
// FT_MOTION_RATE(0.25) from script frame 1 to 9, so at script frame 8 the motion frame is about
// 2.75, and a window of 8..28 in motion-frame terms opened around script frame 13 instead of 8.
// Second and worse, the status End handler cleared the flag -- and End runs at the exact moment
// the hit lands, wiping the flag before the watcher could act on it. That is why Revenge was
// purely cosmetic. ACMD frame() calls are script frames, so driving it from there is exact.
/// How much of a frame passes per game frame during the counter freeze, and for how long.
///
/// APPROXIMATE, AND SAID SO. Every counter in the game freezes both fighters for a beat before the
/// retaliation lands -- playtest: "the native revenge displays some freeze-frames like any other
/// counter showing the counter has been hit before dealing the damage". Ours dealt the damage
/// instantly, which reads as the counter not having registered.
///
/// The exact length is Incineroar's C++. `special_lw_hit_stop_mul` is 1.5, but that multiplies the
/// ATTACKER's hitlag rather than naming a freeze, and with no-reaction armor on there may be no
/// hitlag to multiply. So this is a plain freeze at a plausible counter length, logged when it
/// fires so the next playtest can say "shorter" or "longer" against a number instead of a feeling.
const FREEZE_SLOW_MUL: u8 = 8;
const FREEZE_FRAMES: i32 = 14;

pub unsafe fn open_window(boma: *mut BattleObjectModuleAccessor) {
    let e = entry_of(boma);
    WINDOW_OPEN[e] = true;
    WINDOW_LEFT[e] = WINDOW_FRAMES;
    LAST_DAMAGE[e] = DamageModule::damage(boma, 0);
    // ARMOR, WHICH IS WHY THE COUNTER SOMETIMES DID NOT COUNTER.
    //
    // Playtest: "in certain situations I get a revenge stack but still get normally hit by the
    // move". Exactly so -- the watcher noticed the damage a frame later and refunded it, but by
    // then the engine had already put Pikachu into a damage status and sent him flying. The boost
    // banked; the counter did not happen.
    //
    // Incineroar takes no knockback when Revenge triggers, and the engine has the mechanism for
    // that directly: DAMAGE_NO_REACTION_MODE_ALWAYS for the length of the window. It is the same
    // no-reaction machinery as Little Mac's f-smash armor, reached from Rust instead of from a
    // `damage!` macro. With it on, the hit lands, deals no reaction, and the watcher gets a clean
    // frame to refund the damage and retaliate in.
    DamageModule::set_no_reaction_mode_status(
        boma,
        smash::app::DamageNoReactionMode { _address: *DAMAGE_NO_REACTION_MODE_ALWAYS as u8 },
        0.0,
        0.0,
        0,
    );
    crate::telemetry::probe::event(format!(
        "[rat] REVENGE p{e} window OPEN for {WINDOW_FRAMES}f, armor on; {}",
        crate::telemetry::probe::ctx(boma)
    ));
}

pub unsafe fn close_window(boma: *mut BattleObjectModuleAccessor) {
    let e = entry_of(boma);
    if WINDOW_OPEN[e] {
        crate::telemetry::probe::event(format!(
            "[rat] REVENGE p{e} window CLOSE, armor off; {}",
            crate::telemetry::probe::ctx(boma)
        ));
    }
    WINDOW_OPEN[e] = false;
    // Leaving this on past the window would armor Pikachu through everything, which is a far
    // worse bug than the one it fixes. Reset unconditionally, including on the paths that close
    // the window without the ACMD reaching its frame-28 call.
    DamageModule::reset_no_reaction_mode_status(boma);
}

// Phase within FIGHTER_STATUS_KIND_SPECIAL_LW. Pikachu has no SPECIAL_LW_HIT status kind and
// adding one is not necessary: the retaliation is the same status playing a different motion, so
// one status covers both and there is no new status kind to register.
pub const PHASE_STANCE: i32 = 0;
pub const PHASE_COUNTER: i32 = 1;

pub static mut PHASE: [i32; 8] = [PHASE_STANCE; 8];
pub static mut WINDOW_OPEN: [bool; 8] = [false; 8];
static mut WINDOW_LEFT: [i32; 8] = [0; 8];
pub static mut LAST_DAMAGE: [f32; 8] = [0.0; 8];
pub static mut RATE: [f32; 8] = [1.0; 8];
pub static mut TIMER: [i32; 8] = [0; 8];
pub static mut COUNTERED: [f32; 8] = [0.0; 8];
/// Damage taken since the boost was banked, weighed against CANCEL_PERCENT.
static mut DAMAGE_SINCE_BOOST: [f32; 8] = [0.0; 8];
/// Whether the active-Revenge visual is up, so it is spawned and killed exactly once.
static mut AURA_ON: [bool; 8] = [false; 8];
/// Frames since the visual went up, driving the colour pulse.
static mut AURA_CLOCK: [i32; 8] = [0; 8];

unsafe fn entry_of(boma: *mut BattleObjectModuleAccessor) -> usize {
    let e = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if e >= 8 { 0 } else { e }
}

// THE ACTIVE-REVENGE VISUAL.
//
// Incineroar's is `gaogaen_revenge_aura`, followed on shoulderr, shoulderl, kneer, kneel and
// feelerca -- read straight off his effect_speciallwrevenge script, which his C++ fires when a
// counter banks and cancels from effect_speciallwrevengeend. That effect lives in his own effect
// archive, so ADR 0002 rules it out. The bone list, though, transfers exactly: Pikachu has
// ShoulderL/R and KneeL/R (feelerca is Incineroar's flame whiskers and has no counterpart).
//
// So the aura keeps his structure with a `sys_` effect substituted, and the colour cue is carried
// by FLASH -- which is what Incineroar himself uses on this very move: effect_speciallwstart
// pulses FLASH(0.67, 0, 0.78) purple against grey while he takes the stance. FLASH tints the model
// through ColorBlendModule and needs no archive at all, so it is guaranteed to show even if the
// substituted aura does not. SmashWiki describes the active state as "its fur radiating an orange
// glow", so the pulse here is orange.
const AURA: &str = "sys_final_aura";
const AURA_BONES: [&str; 4] = ["shoulderr", "shoulderl", "kneer", "kneel"];
/// Length of one orange/normal pulse, in frames.
const PULSE: i32 = 24;

unsafe fn aura_on(fighter: &mut L2CFighterCommon) {
    let e = entry_of(fighter.module_accessor);
    if AURA_ON[e] {
        return;
    }
    AURA_ON[e] = true;
    AURA_CLOCK[e] = 0;
    for bone in AURA_BONES {
        macros::EFFECT_FOLLOW(
            &mut **fighter, Hash40::new(AURA), Hash40::new(bone),
            0, 0, 0, 0, 0, 0, 1, false
        );
    }
}

unsafe fn aura_off(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let e = entry_of(boma);
    if !AURA_ON[e] {
        return;
    }
    AURA_ON[e] = false;
    EffectModule::kill_kind(boma, Hash40::new(AURA), false, false);
    macros::COL_NORMAL(&mut **fighter);
}

unsafe fn aura_tick(fighter: &mut L2CFighterCommon) {
    let e = entry_of(fighter.module_accessor);
    let t = AURA_CLOCK[e] % PULSE;
    AURA_CLOCK[e] += 1;
    // Only touch the tint on the two boundary frames, so the ordinary damage flash is not being
    // fought every single frame for the rest of the pulse.
    if t == 0 {
        macros::FLASH(&mut **fighter, 1.0, 0.45, 0.0, 0.35);
    } else if t == PULSE / 2 {
        macros::COL_NORMAL(&mut **fighter);
    }
}

/// Drop a stored boost and everything that goes with it.
unsafe fn clear_boost(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let e = entry_of(boma);
    TIMER[e] = 0;
    RATE[e] = 1.0;
    COUNTERED[e] = 0.0;
    DAMAGE_SINCE_BOOST[e] = 0.0;
    AttackModule::set_power_up(boma, 1.0);
    AttackModule::set_reaction_mul(boma, 1.0);
    aura_off(fighter);
}

unsafe extern "C" fn pikachu_speciallw_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
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
        (*FIGHTER_LOG_MASK_FLAG_ACTION_TRIGGER_ON | *FIGHTER_LOG_MASK_FLAG_ATTACK_KIND_SPECIAL_LW) as u64,
        0,
        FIGHTER_POWER_UP_ATTACK_BIT_SPECIAL_LW.into(),
        0
    );
    0.into()
}

unsafe extern "C" fn pikachu_speciallw_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);

    // The retaliation re-enters this status with PHASE_COUNTER already set, so the same status
    // covers stance and counter-attack without needing a second status kind.
    //
    // AIR OR GROUND, per Incineroar. He has special_air_lw, _start and _turn as well, and this
    // port played the grounded stance and grounded counter for an airborne Revenge. The air stance
    // script is timing-identical to the ground one -- STANCE_START on script frame 8, STANCE_END on
    // 28 -- so the counter window is the same either way and ../acmd_speciallw serves both.
    let air = fighter.global_table[0x16] != *SITUATION_KIND_GROUND;
    let motion = match (PHASE[entry] == PHASE_COUNTER, air) {
        (true, false) => "special_lw",
        (true, true) => "special_air_lw",
        (false, false) => "special_lw_start",
        (false, true) => "special_air_lw_start",
    };
    MotionModule::change_motion(boma, Hash40::new(motion), 0.0, 1.0, false, 0.0, false, false);

    if fighter.global_table[0x16] != *SITUATION_KIND_GROUND {
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_AIR_STOP);
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
    } else {
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_GROUND_STOP);
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
    }
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_speciallw_main_loop as *const () as _))
}

unsafe extern "C" fn pikachu_speciallw_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    // Landing out of an airborne Revenge used to slide across the ground: the status runs on
    // FIGHTER_KINETIC_TYPE_UNIQ, which keeps whatever horizontal momentum the jump had and never
    // brakes. Switching to the grounded kinetic on touchdown hands the momentum back to normal
    // ground friction.
    if StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND
        && KineticModule::get_kinetic_type(boma) == *FIGHTER_KINETIC_TYPE_AIR_STOP
    {
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_GROUND_STOP);
        GroundModule::correct(boma, GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
    }

    if MotionModule::is_end(boma) {
        WINDOW_OPEN[entry] = false;
        PHASE[entry] = PHASE_STANCE;
        if StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND {
            fighter.change_status(FIGHTER_STATUS_KIND_WAIT.into(), false.into());
        } else {
            fighter.change_status(FIGHTER_STATUS_KIND_FALL.into(), false.into());
        }
        return 0.into();
    }
    0.into()
}

unsafe extern "C" fn pikachu_speciallw_end(fighter: &mut L2CFighterCommon) -> L2CValue {
    // Deliberately does NOT clear WINDOW_OPEN. End runs at the moment the counter is hit -- the
    // hit moves Pikachu into a damage status, which ends SPECIAL_LW -- so clearing it here wiped
    // the flag before the watcher could read it, and Revenge never fired. The stance ACMD closes
    // the window on its own frame 28, and the watcher clears it when it fires.
    let _ = fighter;
    0.into()
}

// Runs every frame regardless of status, which is the only place the counter CAN be detected:
// being hit puts the fighter in a damage status, so the SPECIAL_LW status loop stops running at
// exactly the moment the hit lands.
pub unsafe extern "C" fn pikachu_revenge_frame(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    let damage = DamageModule::damage(boma, 0);
    let taken = damage - LAST_DAMAGE[entry];

    // A hit landed while the window was open.
    if WINDOW_OPEN[entry] && WINDOW_LEFT[entry] > 0 && taken > 0.0 {
        // Refund the damage the counter absorbed, then bank the boost. Counters STACK: the new
        // rate builds on whatever is already stored, still capped at revenge_rate_max.
        DamageModule::add_damage(boma, -taken, 0);
        COUNTERED[entry] = taken;
        RATE[entry] = (RATE[entry] + RATE_BASE_ADD + RATE_ATTACK_MUL_ADD * taken).min(RATE_MAX);
        TIMER[entry] = REVENGE_TIME_FRAMES;
        DAMAGE_SINCE_BOOST[entry] = 0.0;
        WINDOW_OPEN[entry] = false;
        PHASE[entry] = PHASE_COUNTER;
        LAST_DAMAGE[entry] = DamageModule::damage(boma, 0);

        // THE FREEZE, which the retaliation had none of. Playtest: "the damage revenge deals
        // after getting hit is being done immediately on hit while the native revenge displays
        // some freeze-frames like any other counter".
        //
        // ON PIKACHU ONLY, DELIBERATELY. A counter pauses both fighters, but the attacker is
        // already in their own hitlag from the hit they just landed, so freezing our side is the
        // half that is missing. Freezing theirs as well needs an entry id turned into a battle
        // object id, and the conversion is not something this codebase can verify offline --
        // guessing it would be a null dereference in a fighter's boma, which is precisely the
        // class of bug that froze the cargo throw.
        //
        // The attacker's entry is logged anyway, so if the one-sided freeze reads wrong the next
        // round starts with the number already in hand.
        let attacker_entry =
            WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_SUCCEED_ATTACKER_ENTRY_ID);
        SlowModule::set_whole(boma, FREEZE_SLOW_MUL, FREEZE_FRAMES);

        // The armor came on with the window; it must come off with the counter or Pikachu keeps
        // it for the whole retaliation.
        DamageModule::reset_no_reaction_mode_status(boma);

        crate::telemetry::probe::event(format!(
            "[rat] REVENGE p{entry} COUNTERED {taken:.1}% -> rate {:.2}, freeze {FREEZE_FRAMES}f \
             at 1/{FREEZE_SLOW_MUL} speed (attacker entry {attacker_entry}); {}",
            RATE[entry],
            crate::telemetry::probe::ctx(boma)
        ));

        StatusModule::change_status_request_from_script(
            boma, *FIGHTER_STATUS_KIND_SPECIAL_LW, false
        );
        return;
    }

    if WINDOW_OPEN[entry] {
        WINDOW_LEFT[entry] -= 1;
        if WINDOW_LEFT[entry] <= 0 {
            WINDOW_OPEN[entry] = false;
        }
    }

    if TIMER[entry] > 0 {
        let status = StatusModule::status_kind(boma);
        let in_counter_attack = PHASE[entry] == PHASE_COUNTER
            && MotionModule::motion_kind(boma) == smash::hash40("special_lw");

        // LOSS CONDITIONS, both Incineroar's own: taking cancel_percent in total while holding a
        // boost destroys it, and so does being thrown.
        if taken > 0.0 {
            DAMAGE_SINCE_BOOST[entry] += taken;
        }
        if DAMAGE_SINCE_BOOST[entry] >= CANCEL_PERCENT
            || status == *FIGHTER_STATUS_KIND_THROWN
            || status == *FIGHTER_STATUS_KIND_CAPTURE_PULLED
            || status == *FIGHTER_STATUS_KIND_CAPTURE_WAIT
        {
            clear_boost(fighter);
            LAST_DAMAGE[entry] = damage;
            return;
        }

        // Two DIFFERENT multipliers, because Incineroar uses two. The retaliation's own damage
        // scales by special_lw_hit_damage_mul (0.4) against what it absorbed, while the STORED
        // rate is what boosts everything afterwards. Applying the stored rate to the retaliation
        // as well would double-count -- a 10% counter would swing it from 1.4x to 2.25x.
        let mul = if in_counter_attack { retaliation_damage_mul(boma) } else { RATE[entry] };
        // power_up scales damage, reaction_mul scales knockback. revenge_data in Incineroar's
        // vl.prc carries reaction_mul 1.15-1.2 per move against power_mul 1, so knockback is the
        // larger part of the effect; both are driven from the same stored rate here.
        AttackModule::set_power_up(boma, mul);
        AttackModule::set_reaction_mul(boma, mul);
        aura_on(fighter);
        aura_tick(fighter);

        // CONSUMED BY THE NEXT LANDED ATTACK, which is the whole point of the move and was the
        // reported bug. The counterattack is exempt: it belongs to Revenge itself and is already
        // scaled by hit_damage_mul, so letting it eat the stored boost would make the buff
        // unusable.
        if !in_counter_attack && AttackModule::is_attack_occur(boma) {
            clear_boost(fighter);
            LAST_DAMAGE[entry] = damage;
            return;
        }

        TIMER[entry] -= 1;
        if TIMER[entry] == 0 {
            clear_boost(fighter);
        }
    }
    LAST_DAMAGE[entry] = damage;
}

// How much extra the retaliation itself does, scaled by what it absorbed. Used by the ACMD
// override so the counter attack is not a flat 2.4 damage regardless of what it countered.
pub unsafe fn retaliation_damage_mul(boma: *mut BattleObjectModuleAccessor) -> f32 {
    let entry = entry_of(boma);
    1.0 + HIT_DAMAGE_MUL * COUNTERED[entry]
}

pub fn install() {
    Agent::new("pikachu")
        .status(Pre,  *FIGHTER_STATUS_KIND_SPECIAL_LW, pikachu_speciallw_pre)
        .status(Main, *FIGHTER_STATUS_KIND_SPECIAL_LW, pikachu_speciallw_main)
        .status(End,  *FIGHTER_STATUS_KIND_SPECIAL_LW, pikachu_speciallw_end)
        .on_line(Main, pikachu_revenge_frame)
        .install();
}
