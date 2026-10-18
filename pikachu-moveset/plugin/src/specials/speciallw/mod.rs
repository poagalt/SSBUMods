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
/// `special_lw_start_x_mul` -- fraction of horizontal speed Revenge keeps on entry.
const START_X_MUL: f32 = 0.6;
/// `special_air_lw_speed_y_mul` -- fraction of vertical speed the airborne one keeps.
const AIR_SPEED_Y_MUL: f32 = 0.7;
/// `special_air_lw_hit_accel_y` from the same object: the airborne stance keeps falling at this
/// accel rather than hanging in place. Recovered by reverse-looking-up the hash40 the real
/// helper passes to `WorkModule::get_param_float` -- it is not a number anyone would have
/// guessed was involved.
const AIR_HIT_ACCEL_Y: f32 = 0.02;

// WHAT THIS PORT DOES NOT REPRODUCE, enumerated because tools/check-srcparams.sh asks and because
// "we did not know" and "we decided not to" should not look the same in six months.
//
// `special_lw_start_x_mul` AND `special_air_lw_speed_y_mul` ARE NOW MODELLED, and what settled
// them was not the param names but the KINETIC TYPE. `_pre` sets FIGHTER_KINETIC_TYPE_UNIQ, which
// HewDraw Remix reproduces for Incineroar verbatim -- and `_main` was then overriding it with
// GROUND_STOP / AIR_STOP, so the entering speed was not being multiplied by 0.6, it was being set
// to zero. With UNIQ kept, the multipliers mean what they say.
//   special_lw_start_x_mul     0.6   fraction of horizontal speed kept
//   special_air_lw_speed_y_mul 0.7   fraction of vertical speed kept, airborne only
//
// `special_lw_hit_stop_mul` IS NOW MODELLED. It is 1.5, and it is the multiplier on the counter
// freeze that HIT_STOP_MUL below applies to Ultimate's hitlag formula -- the round-3 fix for
// "the damage comes out before the hit lands". Listing it as unmodelled was correct until the
// freeze existed to multiply.
//
// `special_air_lw_hit_accel_y` IS NOW MODELLED, at 0.02, and it arrived the same way the two
// above did: `tools/disasm` naming the calls in Incineroar own FUN_7100013e50, then reversing
// the hash40 it passes to `get_param_float` against ParamLabels. It is the airborne stance own
// downward accel, applied to the MOTION energy alongside them.
//
// NOT MODELLED: special_lw_revenge_shield_damage_mul special_lw_revenge_rate_correct_max
// NOT MODELLED: special_lw_revenge_rate_max_team
//   Revenge also builds from shield damage at 0.2x, the stored rate has a separate correction
//   ceiling, and team battles cap it at 1.5 instead of 3.0. The shield case is real behaviour and
//   worth adding; the team cap only matters in a mode this mod is not aimed at.
//
// MODELLED NOW: special_lw_revenge_emmisive_scale_min special_lw_revenge_emmisive_scale_max
//   These were listed here as not modelled because the two values were known and the MAPPING was
//   not. It is now read out of the game: `set_emmisive_scale(min + rate/rate_max * (max - min))`,
//   with the inactive value 1.0 from `revenge_off`. See `revenge_emissive` below for the
//   derivation and the six call sites it was found in. The native call is made with the native
//   value, and the orange tint's alpha now carries the same ramp so the cue is visible on a model
//   whose body material has no emissive slot.
//
// NOT MODELLED: special_lw_revenge_pitch_min special_lw_revenge_pitch_max
// NOT MODELLED: special_lw_revenge_pitch_max_rate
//   The voice pitch scales with the stored rate and would need his voice, which is not shipped.
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
/// THE COUNTER FREEZE, ON THE GAME'S OWN HITLAG FORMULA RATHER THAN A GUESS.
///
/// Playtest, twice: "the damage revenge deals ... is being done immediately on hit while the
/// native revenge displays some freeze-frames like any other counter", and the sequence it should
/// follow -- hit, a few frames of heavy-hit stun for BOTH players, then the fire and the buff.
///
/// Two facts settle the timing, and both come from data rather than from feel.
///
/// The fire is not supposed to be instant even after the freeze: Incineroar's own
/// `game_speciallw` puts its two hitboxes on SCRIPT FRAME 8 (2.4 damage each, angle 51 and 54,
/// `collision_attr_fire`, sizes 10 and 20 on `top`). Our port is byte-identical there, verified
/// by tools/check-acmd-values.sh. So eight frames of wind-up already exist; what was missing is
/// the stun in front of them.
///
/// And Ultimate's hitlag is a published formula, not a mystery:
///
///     hitlag = floor((damage * 0.65 + 6) * h * e * s * p * c),  capped at 30 frames
///
/// with h the hitbox's own multiplier, e 1.5 for electric, and the rest 1 in ordinary play.
/// Incineroar then has `special_lw_hit_stop_mul` = 1.5 -- a param that exists precisely to
/// lengthen the counter's stop and that this module previously listed as NOT MODELLED.
///
/// So the freeze is `floor((absorbed * 0.65 + 6) * 1.5)`, capped at the engine's own 30. A 2.4%
/// jab gives 11 frames, a 15% smash gives 23. That is derived, checkable and tied to what was
/// actually absorbed, which is the behaviour described: the same sequence no matter when in the
/// active frames the hit arrives, scaled by how hard it was.
///
/// Sources: SmashWiki's Hitlag article for the formula, gaogaen's vl.prc for the multiplier.
const HIT_STOP_MUL: f32 = 1.5;

// `counter_freeze_frames` IS GONE. It rebuilt Ultimate's published hitlag formula in Rust so the
// counter could be held for the right number of frames -- correct arithmetic solving a problem
// that Incineroar solves with one call. `HitModule::set_hit_stop_mul` hands the multiplier to the
// engine, which already knows the attack that hit and computes the pause for both fighters.


/// The stored multiplier after countering an attack that dealt `d`.
///
/// BOTH FORMULAS ARE THE GAME'S, TRANSCRIBED. SmashWiki's Revenge article -- kept verbatim at
/// vendor/smashwiki/revenge.wikitext so this can be re-checked without a network -- gives them
/// explicitly, and they are not what this file had:
///
///     first counter    0.075 * d + 1.5
///     with m stored    m + (d * 0.075 + 0.5) * (1 - 0.25 * (m - 1))
///
/// capped at 3x, which a single attack reaches at 20% damage. `d` is the attack's FULL damage,
/// "ignoring the 0.4x multiplier for Incineroar's damage taken".
///
/// The old code was `m + 0.5 + 0.075 * d` for both cases. That is right for the first counter
/// (m starts at 1.0, giving 1.5 + 0.075d) and wrong for every one after it: the real stacking
/// formula DAMPS each addition by how much boost is already stored, so a second counter adds less
/// than the first. Without the damping two small counters overshot, and the article's own worked
/// example -- three 5% attacks giving 1.875x, 2.5586x, then the 3x cap -- is the test case.
///
/// This also fixes a subtler thing: the minimum boost is 1.5x, always. The additive version could
/// produce less than that if RATE had somehow been left below 1.0.

/// Per-category knockback ceilings, straight out of SmashWiki's Revenge table
/// (vendor/smashwiki/revenge.wikitext):
///
///     Neutral attack 1.2   tilts 1.15   dash attack 1.15   smashes 1.2
///     aerials 1.15         throws 1.1   Alolan Whip 1.2    Darkest Lariat 1.1
///     Cross Chop 1.0       Final Smash 1.0                 item swing 1.0
///
/// The same table lists "storage loss (seconds)" per category for whiffing, which is the one
/// documented rule still not reproduced -- whiff detection from a frame hook is not reliable
/// enough to risk draining a boost that should still be there.
///
/// NOT AFFECTED BY REVENGE AT ALL, per the article: pummel, floor attacks, edge attack and
/// non-battering items. They "neither use up the boost nor cause storage loss", which is why
/// they return 1.0 and are excluded from consumption below.
const KB_DEFAULT: f32 = 1.15;
fn category_kb_ceiling(boma: *mut BattleObjectModuleAccessor) -> f32 {
    unsafe {
        let s = StatusModule::status_kind(boma);
        if s == *FIGHTER_STATUS_KIND_ATTACK || s == *FIGHTER_STATUS_KIND_ATTACK_100 {
            1.2
        } else if s == *FIGHTER_STATUS_KIND_ATTACK_S4
            || s == *FIGHTER_STATUS_KIND_ATTACK_HI4
            || s == *FIGHTER_STATUS_KIND_ATTACK_LW4
        {
            1.2
        } else if s == *FIGHTER_STATUS_KIND_THROW {
            1.1
        } else if s == *FIGHTER_STATUS_KIND_SPECIAL_S || s == RAT_SPECIAL_S_ANY {
            // Alolan Whip is 1.2 and Darkest Lariat 1.1; this moveset's side-B IS Alolan Whip.
            1.2
        } else {
            // Tilts, dash attack and every aerial are all 1.15, which is also the safest default
            // for anything unlisted.
            KB_DEFAULT
        }
    }
}

/// Sentinel so the side-B follow-up statuses count as Alolan Whip without listing all five.
const RAT_SPECIAL_S_ANY: i32 = -999;

/// Attacks Revenge does not touch: they neither consume the boost nor get it.
fn revenge_exempt(boma: *mut BattleObjectModuleAccessor) -> bool {
    unsafe {
        let s = StatusModule::status_kind(boma);
        s == *FIGHTER_STATUS_KIND_CATCH_ATTACK
            || s == *FIGHTER_STATUS_KIND_DOWN_STAND_ATTACK
            || s == *FIGHTER_STATUS_KIND_CLIFF_ATTACK
            || s == *FIGHTER_STATUS_KIND_SLIP_STAND_ATTACK
    }
}

fn next_rate(current: f32, d: f32) -> f32 {
    let m = current.max(1.0);
    let raw = if m <= 1.0 {
        RATE_ATTACK_MUL_ADD * d + 1.5
    } else {
        m + (d * RATE_ATTACK_MUL_ADD + RATE_BASE_ADD) * (1.0 - 0.25 * (m - 1.0))
    };
    raw.clamp(1.5, RATE_MAX)
}

/// Knockback multiplier for a stored damage multiplier of `m`, given this move category's ceiling.
///
/// REVENGE'S DAMAGE BOOST IS NOT ITS KNOCKBACK BOOST, and this file applied the damage multiplier
/// to both. SmashWiki: "Revenge's damage boost is ignored entirely by affected attacks. Instead,
/// when Revenge is active, each kind of attack uses its own knockback multiplier, which scales
/// from 1x to the specified amount, proportionally to how close the damage multiplier is to the
/// maximum of 3x."
///
/// So a 3x damage boost on a smash is only 1.2x knockback, and `set_reaction_mul(3.0)` -- which
/// is what this did -- was roughly fifteen times too strong. The scaling is linear in (m-1)/2,
/// which the article's worked example confirms: at m=2, a 1.15x move gives 1.075x and a 1.2x move
/// gives 1.1x.
fn knockback_mul(m: f32, ceiling: f32) -> f32 {
    let t = ((m - 1.0) / (RATE_MAX - 1.0)).clamp(0.0, 1.0);
    1.0 + (ceiling - 1.0) * t
}

pub unsafe fn open_window(boma: *mut BattleObjectModuleAccessor) {
    let e = entry_of(boma);
    WINDOW_OPEN[e] = true;
    // Reset alongside the window, so the probe reports once per stance rather than once ever.
    CANCEL_LOGGED[e] = false;
    FROM_BEHIND[e] = false;
    WINDOW_LEFT[e] = WINDOW_FRAMES;
    LAST_DAMAGE[e] = DamageModule::damage(boma, 0);

    // THESE THREE CALLS ARE INCINEROAR'S, TRANSCRIBED FROM HIS OWN DECOMPILED STATUS SCRIPT.
    //
    // vendor/ssbu-status/.../lua2cpp_gaogaen.nro/gaogaen/gaogaen/status/SpecialLw/main_loop.c,
    // in the `FLAG_STANCE_START` branch. Not inferred from param names, not reconstructed from
    // behaviour -- the same three functions in the same order with the same arguments:
    //
    //     DamageModule__set_damage_mul_2nd_impl(boma, param_special_lw/special_lw_hit_damage_mul)
    //     HitModule__set_hit_stop_mul_impl(boma, param_special_lw/special_lw_hit_stop_mul, 3, 0.0)
    //     DamageModule__set_no_reaction_mode_status_impl(boma, DAMAGE_NO_REACTION_MODE_ALWAYS,
    //                                                    -1.0, -1.0, -1)
    //
    // EVERY ONE OF THEM CORRECTS SOMETHING THIS FILE HAD WRONG:
    //
    //   `set_damage_mul_2nd(0.4)` is how the 0.4 is applied -- the ENGINE scales the incoming
    //   damage. There is nothing to refund. This was calling `add_damage` with a negative number
    //   a frame later, which is a different thing that happens at a different time and shows up
    //   as the percentage jumping and then dropping back.
    //
    //   `set_hit_stop_mul(1.5, 3, 0.0)` is the freeze, and the ENGINE computes its length from
    //   the attack that hit. Round 3 hand-derived Ultimate's hitlag formula and multiplied it by
    //   1.5 -- close in spirit, needless, and wrong whenever the attacker's own hitbox carried a
    //   hitlag multiplier of its own. Handing the multiplier to the engine is both exact and less
    //   code. `3` and `0.0` are Incineroar's literal arguments.
    //
    //   `set_no_reaction_mode_status(ALWAYS, -1.0, -1.0, -1)` takes MINUS ONE for all three
    //   trailing arguments. This was passing 0.0, 0.0, 0 -- and round 3's log shows Pikachu in
    //   `damage_air_1` when the counter fired, which is the armor not applying. A zero threshold
    //   almost certainly armors nothing; -1 is the sentinel for "no limit".
    let dmg_mul = WorkModule::get_param_float(boma, smash::hash40("param_special_lw"),
                                              smash::hash40("special_lw_hit_damage_mul"));
    let stop_mul = WorkModule::get_param_float(boma, smash::hash40("param_special_lw"),
                                               smash::hash40("special_lw_hit_stop_mul"));
    // Pikachu has no param_special_lw, so a lookup returns 0 and the constants stand in. Reading
    // first anyway costs nothing and means a future shipped param object is picked up for free.
    let dmg_mul = if dmg_mul > 0.0 { dmg_mul } else { HIT_DAMAGE_MUL };
    let stop_mul = if stop_mul > 0.0 { stop_mul } else { HIT_STOP_MUL };

    ABSORB_ARMED[e] = true;
    DamageModule::set_damage_mul_2nd(boma, dmg_mul);
    HitModule::set_hit_stop_mul(boma, stop_mul, smash::app::HitStopMulTarget { _address: 3 }, 0.0);
    DamageModule::set_no_reaction_mode_status(
        boma,
        smash::app::DamageNoReactionMode { _address: *DAMAGE_NO_REACTION_MODE_ALWAYS as u8 },
        -1.0,
        -1.0,
        -1,
    );
    crate::telemetry::probe::event(format!(
        "[rat] REVENGE p{e} window OPEN {WINDOW_FRAMES}f; damage_mul_2nd={dmg_mul} \
         hit_stop_mul={stop_mul} no_reaction=ALWAYS(-1,-1,-1); {}",
        crate::telemetry::probe::ctx(boma)
    ));
}

pub unsafe fn close_window(boma: *mut BattleObjectModuleAccessor) {
    let e = entry_of(boma);
    if WINDOW_OPEN[e] {
        crate::telemetry::probe::event(format!(
            "[rat] REVENGE p{e} window CLOSE, reverting all three; {}",
            crate::telemetry::probe::ctx(boma)
        ));
    }
    WINDOW_OPEN[e] = false;
    revert_absorb(boma);
}

/// Put the three absorb calls back, exactly as Incineroar does.
///
/// THIS IS THE WHOLE OF `SpecialLw_end` IN THE DECOMP, and it runs UNCONDITIONALLY on leaving the
/// status, for any reason:
///
///   set_no_reaction_mode_status(DAMAGE_NO_REACTION_MODE_NORMAL, -1.0, -1.0, -1)
///   set_damage_mul_2nd(1.0)
///   set_hit_stop_mul(1.0, 3, 0.0)
///
/// and nothing else -- it does not touch any window flag. That split is the point. Our `End`
/// handler used to do nothing at all, on the reasoning that clearing state there had once wiped
/// WINDOW_OPEN before the watcher could read it and Revenge never fired. The reasoning was right
/// about the flag and wrong about everything else: the three ENGINE calls have to be reverted or
/// they are still armed after the stance ends, which leaves Pikachu on 0.4x incoming damage with
/// DAMAGE_NO_REACTION_MODE_ALWAYS -- permanently, until the next Revenge happens to close its
/// window cleanly. Halved damage and total flinch immunity for the rest of the stock.
///
/// So: the flag is the watcher's, the three calls are the status's, and only `close_window`
/// touches both.
///
/// AND IT IS ONLY `SpecialLw_end` THAT DOES THIS. `SpecialLwHit_end` and `SpecialLwTurn_end` are
/// both literally `return 0` -- by the time either retaliation starts, the stance's own end has
/// already reverted all three. One function is registered on all three statuses here anyway, for
/// two reasons: on the native path the extra call writes the values that are already set, so it is
/// a no-op; and the port's PHASE and WINDOW_OPEN bookkeeping has to be cleared when the
/// RETALIATION ends, which is a thing Incineroar has no equivalent of because his motion choice
/// comes from the status rather than from a variable.
unsafe fn revert_absorb(boma: *mut BattleObjectModuleAccessor) {
    ABSORB_ARMED[entry_of(boma)] = false;
    DamageModule::set_no_reaction_mode_status(
        boma,
        smash::app::DamageNoReactionMode { _address: *DAMAGE_NO_REACTION_MODE_NORMAL as u8 },
        -1.0,
        -1.0,
        -1,
    );
    DamageModule::set_damage_mul_2nd(boma, 1.0);
    HitModule::set_hit_stop_mul(boma, 1.0, smash::app::HitStopMulTarget { _address: 3 }, 0.0);
}

// Phase within FIGHTER_STATUS_KIND_SPECIAL_LW. Pikachu has no SPECIAL_LW_HIT status kind and
// adding one is not necessary: the retaliation is the same status playing a different motion, so
// one status covers both and there is no new status kind to register.
pub const PHASE_STANCE: i32 = 0;
pub const PHASE_COUNTER: i32 = 1;

pub static mut PHASE: [i32; 8] = [PHASE_STANCE; 8];
/// One `is_enable_cancel` line per stance, not one per frame. See the probe in the main loop.
static mut CANCEL_LOGGED: [bool; 8] = [false; 8];
pub static mut WINDOW_OPEN: [bool; 8] = [false; 8];
static mut WINDOW_LEFT: [i32; 8] = [0; 8];
pub static mut LAST_DAMAGE: [f32; 8] = [0.0; 8];
pub static mut RATE: [f32; 8] = [1.0; 8];
pub static mut TIMER: [i32; 8] = [0; 8];
pub static mut COUNTERED: [f32; 8] = [0.0; 8];
/// Damage taken since the boost was banked, weighed against CANCEL_PERCENT.
static mut DAMAGE_SINCE_BOOST: [f32; 8] = [0.0; 8];
/// Did the hit that opened the retaliation come from behind? Written by `revenge_check_damage`.
static mut FROM_BEHIND: [bool; 8] = [false; 8];
/// Has `revenge_check_damage` ever run for this entry? Reported once, and never reset -- the
/// question is whether the line is installed at all, which does not change during a boot.
static mut CHECK_DMG_RAN: [bool; 8] = [false; 8];
/// Whether the active-Revenge visual is up, so it is spawned and killed exactly once.
/// Whether the three absorb calls are currently armed on this entry.
///
/// A SHADOW OF ENGINE STATE, BECAUSE THE ENGINE WILL NOT TELL US. There is no getter for
/// `damage_mul_2nd` or for the no-reaction mode -- `DamageModule` exposes setters and
/// `is_no_reaction_mode_perfect` and nothing else -- so the only way to notice that we left
/// Pikachu armed is to track our own writes and check the invariant every frame. See
/// `watch_absorb_leak`.
static mut ABSORB_ARMED: [bool; 8] = [false; 8];

/// The last Revenge multipliers actually written to the engine, per entry.
///
/// WHY THIS CACHE EXISTS. The pair below used to be written on EVERY frame the boost was live,
/// re-sending a value the engine already held. That is the shape of defect that caused the
/// air-damage crash on 2026-10-17 -- `retarget_forward_throw` writing `globalTable[0x45..0x48]`
/// every frame in every status -- and while these two are far safer (they are per-fighter slots
/// behind a Pikachu guard, not a shared L2C table), re-telling the engine something it already
/// knows is not how the game does it. `FighterSpecializer_Gaogaen` sets these at the transitions
/// and resets them in `revenge_off` (`main 0xab86f0`); it does not poll.
///
/// NaN is the "nothing written yet" marker rather than 1.0, because 1.0 is a REAL value here --
/// the identity the counter-attack frames write -- and seeding the cache with it would swallow
/// the first genuine write of it.
static mut LAST_POWER_MUL: [f32; 8] = [f32::NAN; 8];
static mut LAST_REACTION_MUL: [f32; 8] = [f32::NAN; 8];

/// Write the Revenge multipliers only when they actually change.
unsafe fn set_revenge_muls(
    boma: *mut BattleObjectModuleAccessor,
    entry: usize,
    power: f32,
    reaction: f32,
) {
    if LAST_POWER_MUL[entry].to_bits() != power.to_bits() {
        LAST_POWER_MUL[entry] = power;
        AttackModule::set_power_mul_5th(boma, power);
    }
    if LAST_REACTION_MUL[entry].to_bits() != reaction.to_bits() {
        LAST_REACTION_MUL[entry] = reaction;
        AttackModule::set_reaction_mul_3rd(boma, reaction);
    }
}

/// Is the counter window's `DAMAGE_NO_REACTION_MODE_ALWAYS` currently armed?
///
/// `crate::cues` asks, because its armored-hit detector is "damage rose with no reaction status" --
/// and that is exactly what Revenge's stance does on purpose for its whole window. Absorbing there
/// is the move working, and it already has its own aura and its own counter cue, so it must not
/// also fire the armor cue.
pub unsafe fn absorb_armed(entry: usize) -> bool {
    entry < 8 && ABSORB_ARMED[entry]
}
/// Last frame's `AttackModule::is_attack_occur`, so the consumption can be charged on the rising
/// edge. The flag is sticky for the whole move, which is why reading it directly spent a boost on
/// the counter's own fire -- see the consumption test.
static mut ATTACK_SEEN: [bool; 8] = [false; 8];
static mut AURA_ON: [bool; 8] = [false; 8];

unsafe fn entry_of(boma: *mut BattleObjectModuleAccessor) -> usize {
    let e = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if e >= 8 { 0 } else { e }
}

// THE ACTIVE-REVENGE VISUAL, AS INCINEROAR'S OWN, VERBATIM (ADR 0007).
//
// His C++ fires `effect_speciallwrevenge` when a counter banks and `effect_speciallwrevengeend`
// when it is spent (`FighterSpecializer_Gaogaen::revenge_off`, main 0xab86f0, calls the latter by
// hash 0x1a264dbb63). The whole of `lua2cpp_gaogaen/gaogaen/SpecialLwRevenge.txt` is:
//
//     EFFECT_OFF_KIND(gaogaen_revenge_aura, false, false)
//     EFFECT_FOLLOW(gaogaen_revenge_aura, shoulderr, 0, 0, 0, 0, 0, 0, 1, false)
//     EFFECT_FOLLOW(gaogaen_revenge_aura, shoulderl, ...)   kneer, kneel: the same
//     EFFECT_FOLLOW(gaogaen_revenge_aura, feelerca,  0, 0, 3, 0, 0, 0, 1, false)
//
// and the end script is that one EFFECT_OFF_KIND. Until 2026-10-18 this ran a `sys_final_aura`
// stand-in with an orange FLASH pulse, because ADR 0002 kept his archive out; Rat of Doom now ships
// `gaogaen_revenge_aura` in its transplanted archive (tools/build-effects.py finds it here), so the
// stand-in and the pulse -- which the native move does not have -- are gone.
//
// `feelerca` is Incineroar's flame whiskers, and Pikachu's nearest point is his `head`. That one
// joint is the only substitution; the other four are the same bones by name on both rigs.
//
// BIGGER THAN NATIVE, ON PURPOSE (2026-10-18, playtest: "a bit hard to see in pikachu's frame").
// The effect is sized for Incineroar, and on Pikachu's frame Incineroar's five points sit close
// together and half inside the body. So every point is scaled up by AURA_SIZE, and two points are
// added where Pikachu has mass Incineroar's layout misses: the hip (his centre) and the tail.
// THE NAME IS WRITTEN IN EACH CALL, not held in a constant: tools/build-effects.py and gate 4o
// read effect names out of the call's own arguments, so a name behind a constant is invisible to
// both -- the first build of the transplant shipped without this effect for exactly that reason.
const AURA_BONES: [(&str, f32); 7] = [
    ("shoulderr", 0.0),
    ("shoulderl", 0.0),
    ("kneer", 0.0),
    ("kneel", 0.0),
    ("head", 3.0),
    ("hip", 0.0),
    ("tail3", 0.0),
];
const AURA_SIZE: f32 = 1.5;

/// One live handle per AURA_BONES point, per entry. 0 = not requested.
static mut AURA_HANDLE: [[u32; 7]; 8] = [[0; 7]; 8];
/// The status in which the last re-request was logged, so a lost aura is reported once per status
/// rather than once per frame.
static mut AURA_LOST_LOGGED: [i32; 8] = [-1; 8];

unsafe fn aura_request(fighter: &mut L2CFighterCommon, i: usize) {
    let (bone, z) = AURA_BONES[i];
    macros::EFFECT_FOLLOW(
        &mut **fighter, Hash40::new("gaogaen_revenge_aura"), Hash40::new(bone),
        0, 0, z, 0, 0, 0, AURA_SIZE, false
    );
    let e = entry_of(fighter.module_accessor);
    AURA_HANDLE[e][i] = EffectModule::get_last_handle(fighter.module_accessor) as u32;
}

/// THE AURA HOLDS FOR AS LONG AS THE BOOST DOES. Incineroar requests it once and his moves leave it
/// alone; on Pikachu something in the borrowed moves does not -- playtest: it has to "persist
/// through skeleton changes for other characters' moves". Rather than chase each one, every frame
/// a boost is banked checks each of the seven handles and re-requests whichever is gone, and says
/// in which status and motion it was lost, so the cause is in the next log.
unsafe fn aura_on(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let e = entry_of(boma);
    if !AURA_ON[e] {
        AURA_ON[e] = true;
        AURA_LOST_LOGGED[e] = -1;
        macros::EFFECT_OFF_KIND(&mut **fighter, Hash40::new("gaogaen_revenge_aura"), false, false);
        for i in 0..AURA_BONES.len() {
            aura_request(fighter, i);
        }
        return;
    }
    let mut lost: Vec<&str> = Vec::new();
    for i in 0..AURA_BONES.len() {
        let h = AURA_HANDLE[e][i];
        if h == 0 || !EffectModule::is_exist_effect(boma, h) {
            lost.push(AURA_BONES[i].0);
            aura_request(fighter, i);
        }
    }
    if !lost.is_empty() {
        let status = StatusModule::status_kind(boma);
        if AURA_LOST_LOGGED[e] != status {
            AURA_LOST_LOGGED[e] = status;
            use crate::telemetry::probe;
            probe::event(format!(
                "[rat] REVENGE p{e} aura LOST on {lost:?} in {} -- re-requested; {}",
                probe::st(status),
                probe::ctx(boma)
            ));
        }
    }
}

unsafe fn aura_off(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let e = entry_of(boma);
    if !AURA_ON[e] {
        return;
    }
    AURA_ON[e] = false;
    AURA_HANDLE[e] = [0; 7];
    // `effect_speciallwrevengeend`, the whole of it.
    macros::EFFECT_OFF_KIND(&mut **fighter, Hash40::new("gaogaen_revenge_aura"), false, false);
    // `FighterSpecializer_Gaogaen::revenge_off` (`main 0xab86f0`) ends its visual teardown with
    // `ModelModule::set_emmisive_scale(1.0)` alongside resetting `set_power_mul_5th` and
    // `set_reaction_mul_3rd` -- both of which this function already mirrors in `clear_boost`. 1.0
    // is therefore the inactive value, read off the game rather than chosen.
    ModelModule::set_emmisive_scale(boma, 1.0);
}

/// THE NATIVE EMISSIVE LAW, recovered from the game rather than guessed.
///
/// This file used to list `special_lw_revenge_emmisive_scale_min` and `_max` as NOT MODELLED, and
/// `docs/the-weapons.md` recorded them as two param values with "no evidence for how they are
/// interpolated", which is not enough to implement under ADR 0005. There is evidence now.
///
/// `ModelModule::set_emmisive_scale` is vtable `0x258` on the module at `boma+0x78` -- checked, not
/// assumed: `ModelModule__set_emmisive_scale_impl` (`main 0x20577a0`) is four instructions,
/// `ldr x0,[x0,#0x78]` / `ldr x8,[x0]` / `ldr x1,[x8,#0x258]` / `br x1`. That slot is called from
/// exactly SIX places in the whole game, five of them Incineroar's, and two of those are
///
///     set_emmisive_scale(fVar9 + fVar16 * (fVar15 - fVar9))
///
/// with `fVar9` = `special_lw_revenge_emmisive_scale_min`, `fVar15` = `_max`, and `fVar16` =
/// `WorkModule::get_float(0x4e) / special_lw_revenge_rate_max` -- every one of those three param
/// hashes confirmed by hashing the name back (`0x25f3eddc2e`, `0x25cfe0e377`, `0x1b3f8ea8af`).
/// `revenge_off` sets it to 1.0, which is the inactive value that was also recorded as unknown.
///
/// So the glow is not a fixed pulse: **it tracks how much damage is banked**, ramping as the rate
/// climbs toward its cap. That is a real cue for the opponent and it is the one this port was
/// approximating with a constant.
unsafe fn revenge_emissive(boma: *mut BattleObjectModuleAccessor, rate: f32) -> f32 {
    let obj = hash40("param_special_lw");
    let lo = WorkModule::get_param_float(boma, obj, hash40("special_lw_revenge_emmisive_scale_min"));
    let hi = WorkModule::get_param_float(boma, obj, hash40("special_lw_revenge_emmisive_scale_max"));
    let cap = WorkModule::get_param_float(boma, obj, hash40("special_lw_revenge_rate_max"));
    let t = if cap > 0.0 { (rate / cap).max(0.0).min(1.0) } else { 0.0 };
    lo + t * (hi - lo)
}

unsafe fn aura_tick(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let e = entry_of(boma);

    // THE NATIVE CALL, WITH THE NATIVE VALUE. Pikachu's body has no emissive slot
    // (`skin_pikachu_001` carries Texture0/4/6/7), so on him the visible cue is the aura above; it
    // is called anyway because it is what the game does, and the merged Mythra blade's
    // `def_light_004_emi`/`_005_emi` really do glow with the banked rate while it is out.
    let emissive = revenge_emissive(boma, RATE[e]);
    ModelModule::set_emmisive_scale(boma, emissive);
}

/// Drop a stored boost and everything that goes with it.
unsafe fn clear_boost(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let e = entry_of(boma);
    TIMER[e] = 0;
    RATE[e] = 1.0;
    COUNTERED[e] = 0.0;
    DAMAGE_SINCE_BOOST[e] = 0.0;
    // UNCONDITIONAL, and the cache is invalidated first. Teardown has to reach the engine even if
    // the cache thinks 1.0 is already there -- a stale cache across a death or a re-entry would
    // otherwise leave the boost applied with nothing tracking it.
    LAST_POWER_MUL[e] = f32::NAN;
    LAST_REACTION_MUL[e] = f32::NAN;
    AttackModule::set_power_mul_5th(boma, 1.0);
    AttackModule::set_reaction_mul_3rd(boma, 1.0);
    aura_off(fighter);
}

unsafe extern "C" fn pikachu_speciallw_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    revenge_pre(fighter, true)
}

/// THE THREE `pre`s DIFFER BY ONE FLAG, AND THIS PORT HAD ALL THREE PASSING THE STANCE'S.
///
/// `SpecialLwHit_pre` and `SpecialLwTurn_pre` are byte-for-byte the stance's except for the log
/// mask: the stance passes ATTACK_KIND_SPECIAL_LW | ACTION_CATEGORY_ATTACK | ACTION_TRIGGER_ON and
/// the two retaliations pass only the first two. `pikachu_speciallwhit_pre` was a straight call to
/// the stance's, so the retaliation was being filed in the engine's action log as a
/// player-triggered action, which it is not -- the engine triggered it.
///
///     grep -nE "LOG_MASK" .../status/SpecialLw{,Hit,Turn}/pre.c
///     SpecialLw/pre.c:70      ... | ACTION_CATEGORY_ATTACK | FIGHTER_LOG_MASK_FLAG_ACTION_TRIGGER_ON
///     SpecialLwHit/pre.c:70   ... | ACTION_CATEGORY_ATTACK)
///     SpecialLwTurn/pre.c:70  ... | ACTION_CATEGORY_ATTACK)
unsafe fn revenge_pre(fighter: &mut L2CFighterCommon, trigger_on: bool) -> L2CValue {
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
        // ACTION_CATEGORY_ATTACK was missing, and the decomp has it:
        // SpecialLw/pre.c passes ATTACK_KIND_SPECIAL_LW | ACTION_CATEGORY_ATTACK |
        // ACTION_TRIGGER_ON. The category is how the engine files the move in its own action log,
        // which is what training-mode counters and the game's statistics read.
        //
        // The ATTR argument stays 0 on purpose. HewDraw Remix's gaogaen special_lw_pre passes
        // FIGHTER_STATUS_ATTR_START_TURN here and vanilla does NOT -- SpecialS/pre.c has it,
        // SpecialLw/pre.c has no attr line at all. HDR is a rebalance mod, so where it and the
        // decomp disagree the decomp is the source of truth; see vendor/hdr/README-rat.md.
        (*FIGHTER_LOG_MASK_FLAG_ATTACK_KIND_SPECIAL_LW
            | *FIGHTER_LOG_MASK_FLAG_ACTION_CATEGORY_ATTACK
            | if trigger_on { *FIGHTER_LOG_MASK_FLAG_ACTION_TRIGGER_ON } else { 0 }) as u64,
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

    let air = fighter.global_table[0x16] != *SITUATION_KIND_GROUND;
    GroundModule::correct(
        boma,
        GroundCorrectKind(if air { *GROUND_CORRECT_KIND_AIR } else { *GROUND_CORRECT_KIND_GROUND_CLIFF_STOP }),
    );
    revenge_kinetics(fighter, true);
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_speciallw_main_loop as *const () as _))
}

/// Incineroar's `FUN_7100013e50`, transcribed WHOLE. All three Revenge statuses call it, on entry
/// with `true` and on a mid-status situation change with `false`.
///
/// WHAT IT WAS BEFORE, AND WHY THAT MATTERED. The previous version was a nine-call sketch built on
/// one real finding and three guesses. The finding was right and is kept: the entering speed had
/// been written to `FIGHTER_KINETIC_ENERGY_ID_CONTROL`, which is the PLAYER'S energy -- the thing
/// the stick drives -- so it damped nothing and handed the stance straight back to the controller
/// ("allows the player to slide around rather than killing their momentum and locking them in
/// place until revenge is over"). Moving it off CONTROL cured that.
///
/// The guesses were all three wrong, and `--trace` on the real helper -- 50 calls against the
/// port's nine -- says so:
///
///   * `change_kinetic(UNIQ)` should be GROUND_STOP or FALL on the situation. `UNIQ` is what the
///     three `pre`s hand `init_settings`, and it was reused here.
///   * the entering horizontal speed belongs to the STOP energy, not MOTION. It arrives as the
///     THIRD pushed argument of `reset_energy(STOP, type, sx, 0, 0, 0, 0)` -- the sx slot at
///     0x71000146bc, 0x710001476c, 0x7100014ba8 and 0x7100014c58. MOTION is reset to five literal
///     zeros at all four of its call sites and never given a speed at all.
///   * the two `special_air_lw_*` params act on the GRAVITY energy, which this port did not touch.
///   * `clear_unable_energy` is called with CONTROL, and with GRAVITY in the grounded branch.
///     CONTROL_NO_STOP, which the old comment justified as "its no-stop twin, the pair that
///     survives a hitstop", appears nowhere in the function.
///
/// AND IT IS A THREE-WAY ON THE STATUS, not one shape parameterised by air. `status == SPECIAL_LW`
/// and `status == SPECIAL_LW_HIT || SPECIAL_LW_TURN` take different paths, and anything else
/// returns immediately -- the same two comparison sites (0x7100014148, 0x7100014174) that proved
/// the HIT/TURN transitions are engine-side.
///
/// Every constant here is read off an `ldr wN, [x24, #off]` and named through
/// `vendor/ssbu-status/lua_dict.txt`: 0x410 GROUND_STOP, 0x430 FALL, 0x3b4 GRAVITY, 0x498 STOP,
/// 0x49c/0x4a4 ENERGY_STOP_RESET_TYPE_GROUND/_AIR, 0x274 MOTION, 0x280c/0x2810
/// ENERGY_MOTION_RESET_TYPE_GROUND_TRANS/_AIR_TRANS, 0x4b0 ENERGY_GRAVITY_RESET_TYPE_GRAVITY,
/// 0x4a8 CONTROL, 0x348 KINETIC_ENERGY_RESERVE_ATTRIBUTE_MAIN, 0xb608 the air-speed latch.
///
/// The three params keep their values, which were already right -- reverse-looked-up from their
/// hash40s and read out of `gaogaen/param/vl.prc`:
///
///   special_lw_start_x_mul       0.6     fraction of entering horizontal speed the STANCE keeps
///   special_air_lw_speed_y_mul   0.7     fraction of entering vertical speed, ONCE per life
///   special_air_lw_hit_accel_y   0.02    negated, and only on the RETALIATIONS
///
/// what changed is where each of them is applied.
unsafe fn revenge_kinetics(fighter: &mut L2CFighterCommon, flag: bool) {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);

    // The five values the real helper hoists before its dispatch, in its order. Pikachu has no
    // `param_special_lw`, so the three multipliers are the constants at the top of this file --
    // read out of `gaogaen/param/vl.prc` -- rather than a `get_param_float` that returns 0.
    let mut sx = KineticModule::get_sum_speed_x(boma, *KINETIC_ENERGY_RESERVE_ATTRIBUTE_MAIN);
    let sy = KineticModule::get_sum_speed_y(boma, *KINETIC_ENERGY_RESERVE_ATTRIBUTE_MAIN);

    let status = fighter.global_table[0x9].get_i32();
    let ground = fighter.global_table[0x16] == *SITUATION_KIND_GROUND;

    // 0x7100014058. NOT `UNIQ` -- that is what the three `pre`s hand `init_settings`, and reusing
    // it here was this function's oldest mistake.
    KineticModule::change_kinetic(
        boma,
        if ground { *FIGHTER_KINETIC_TYPE_GROUND_STOP } else { *FIGHTER_KINETIC_TYPE_FALL },
    );

    let retaliation =
        status == RAT_STATUS_KIND_SPECIAL_LW_HIT || status == RAT_STATUS_KIND_SPECIAL_LW_TURN;
    if !(status == *FIGHTER_STATUS_KIND_SPECIAL_LW || retaliation) {
        // 0x710001419c falls straight out. Nothing else in the function runs.
        return;
    }

    // ---- THE GRAVITY ENERGY ----
    //
    // Grounded, both branches only re-enable whatever the previous status had disabled. Airborne,
    // they reset it and write a vertical speed -- and this is the energy the two `special_air_lw_*`
    // params act on, which is why applying them to MOTION never behaved.
    if ground {
        KineticUtility::clear_unable_energy(*FIGHTER_KINETIC_ENERGY_ID_GRAVITY, boma);
    } else {
        sv_kinetic_energy!(
            reset_energy, fighter, *FIGHTER_KINETIC_ENERGY_ID_GRAVITY,
            *ENERGY_GRAVITY_RESET_TYPE_GRAVITY, 0.0, 0.0, 0.0, 0.0, 0.0
        );
        if retaliation {
            // 0x71000144f0: entering the retaliation kills the vertical speed outright; a
            // situation change mid-retaliation keeps it.
            let vy = if flag { 0.0 } else { sy };
            sv_kinetic_energy!(set_speed, fighter, *FIGHTER_KINETIC_ENERGY_ID_GRAVITY, vy);
            // `_ZNK3lib8L2CValuengEv` at 0x7100014558 and 0x7100014aa4 -- operator-, on the param.
            sv_kinetic_energy!(
                set_accel, fighter, *FIGHTER_KINETIC_ENERGY_ID_GRAVITY, -AIR_HIT_ACCEL_Y
            );
        } else if flag && !AIR_SPEED_Y_SPENT[entry] {
            // 0x7100014394: `sy * special_air_lw_speed_y_mul`, ONCE, latched behind
            // FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLAG_INVALID_SPECIAL_AIR_LW_SPPED_Y (0x200000E6).
            sv_kinetic_energy!(
                set_speed, fighter, *FIGHTER_KINETIC_ENERGY_ID_GRAVITY, sy * AIR_SPEED_Y_MUL
            );
            AIR_SPEED_Y_SPENT[entry] = true;
        } else {
            sv_kinetic_energy!(set_speed, fighter, *FIGHTER_KINETIC_ENERGY_ID_GRAVITY, sy);
        }
        KineticModule::enable_energy(boma, *FIGHTER_KINETIC_ENERGY_ID_GRAVITY);
    }

    // 0x71000145f4, and only in the stance: `sx = sx * special_lw_start_x_mul`, IN PLACE, so the
    // value the STOP energy is about to be reset with is the scaled one.
    if !retaliation && flag {
        sx *= START_X_MUL;
    }

    // ---- THE STOP ENERGY, WHICH IS WHERE THE ENTERING HORIZONTAL SPEED GOES ----
    //
    // `reset_energy(STOP, type, sx, 0, 0, 0, 0)` -- the third pushed argument at 0x71000146bc,
    // 0x710001476c, 0x7100014ba8 and 0x7100014c58 is the sx slot in all four places. This port put
    // that speed on MOTION, which is a different energy with a different decay.
    sv_kinetic_energy!(
        reset_energy, fighter, *FIGHTER_KINETIC_ENERGY_ID_STOP,
        if ground { *ENERGY_STOP_RESET_TYPE_GROUND } else { *ENERGY_STOP_RESET_TYPE_AIR },
        sx, 0.0, 0.0, 0.0, 0.0
    );
    KineticModule::enable_energy(boma, *FIGHTER_KINETIC_ENERGY_ID_STOP);

    // ---- THE MOTION ENERGY, RESET TO NOTHING ----
    //
    // All five pushed values are literal zeros in every one of the four call sites. The animation
    // supplies the movement; this only clears whatever the previous status left in it.
    sv_kinetic_energy!(
        reset_energy, fighter, *FIGHTER_KINETIC_ENERGY_ID_MOTION,
        if ground { *ENERGY_MOTION_RESET_TYPE_GROUND_TRANS } else { *ENERGY_MOTION_RESET_TYPE_AIR_TRANS },
        0.0, 0.0, 0.0, 0.0, 0.0
    );
    // 0x7100014efc / 0x7100014f28: `!flag && status == SPECIAL_LW_TURN`. A situation change during
    // the turn re-plays the animation with `change_motion_inherit_frame`, and its root motion runs
    // the other way once the script's REVERSE_LR has fired -- so the motion energy is negated.
    if !flag && status == RAT_STATUS_KIND_SPECIAL_LW_TURN {
        sv_kinetic_energy!(set_speed_mul, fighter, *FIGHTER_KINETIC_ENERGY_ID_MOTION, -1.0);
    }
    KineticModule::enable_energy(boma, *FIGHTER_KINETIC_ENERGY_ID_MOTION);

    // 0x7100014a4c / 0x7100014fdc, the last call on both paths. ONE id, not two: the port's second
    // call passed FIGHTER_KINETIC_ENERGY_ID_CONTROL_NO_STOP on the reasoning that Incineroar
    // "clears twice", and he does -- but the first of the two is the GRAVITY clear above, in the
    // grounded branch. CONTROL_NO_STOP appears nowhere in the helper.
    KineticUtility::clear_unable_energy(*FIGHTER_KINETIC_ENERGY_ID_CONTROL, boma);

    crate::telemetry::probe::event(format!(
        "[rat] REVENGE kinetics st={status:#x} flag={flag} ground={ground} entering \
         ({sx:.2},{sy:.2}) -> kinetic {} / STOP.x {sx:.2}; {}",
        if ground { "GROUND_STOP" } else { "FALL" },
        crate::telemetry::probe::ctx(boma)
    ));
}

/// `FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLAG_INVALID_SPECIAL_AIR_LW_SPPED_Y` (0x200000E6, and the
/// typo is Nintendo's).
///
/// A once-only latch on the airborne stance's `special_air_lw_speed_y_mul`, and it really is
/// once-only: `--scan 0xb608` finds the constant at exactly two sites in the whole gaogaen NRO,
/// the `is_flag` and the `on_flag` inside this one helper, and `revenge_off` clears 0x200000E8,
/// E3 and E4 but not E6. So within one life the FIRST airborne Revenge keeps 70% of its vertical
/// speed and every later one keeps all of it. That reads oddly and it is what the code says.
///
/// Cleared on `ObjectEvent::Start` because that is where the engine's own instance-work array is
/// zeroed, and a `static mut` would otherwise carry a previous stock's latch into the next one --
/// the trap `whip::state::reset` names.
static mut AIR_SPEED_Y_SPENT: [bool; 8] = [false; 8];

unsafe extern "C" fn revenge_object_start(fighter: &mut L2CFighterCommon) {
    let entry = entry_of(fighter.module_accessor);
    if entry < 8 {
        AIR_SPEED_Y_SPENT[entry] = false;
    }
}
/// Revenge's RETALIATION status. Incineroar's `SpecialLwHit`: identical shape to SpecialLw, same
/// UNIQ kinetic and the same movement helper, differing only in the motion it plays.
unsafe extern "C" fn pikachu_speciallwhit_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    revenge_pre(fighter, false)
}

unsafe extern "C" fn pikachu_speciallwhit_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let air = fighter.global_table[0x16] != *SITUATION_KIND_GROUND;
    let motion = if air { "special_air_lw" } else { "special_lw" };
    MotionModule::change_motion(boma, Hash40::new(motion), 0.0, 1.0, false, 0.0, false, false);
    GroundModule::correct(
        boma,
        GroundCorrectKind(if air { *GROUND_CORRECT_KIND_AIR } else { *GROUND_CORRECT_KIND_GROUND_CLIFF_STOP }),
    );
    revenge_kinetics(fighter, true);
    crate::telemetry::probe::event(format!(
        "[rat] REVENGE retaliation status entered, motion '{motion}'; {}",
        crate::telemetry::probe::ctx(boma)
    ));
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_speciallw_main_loop as *const () as _))
}

/// Revenge's retaliation FACING THE OTHER WAY. Incineroar's `SpecialLwTurn`.
///
/// The same shape again -- `SpecialLwTurn/main.c` differs from `SpecialLwHit/main.c` only in the
/// two motion hashes, which are the source's own literals resolved rather than assumed:
///
///     0x0fa4760a42  special_lw_turn
///     0x13afef8796  special_air_lw_turn
///
/// The turn itself is not done here. `game_speciallwturn` carries a `REVERSE_LR` alongside its two
/// hitboxes, so the flip is the script's, on the script's frame -- which is why this status has no
/// `reverse_lr` call and why the ACMD has been shipping complete since the counter was ported.
unsafe extern "C" fn pikachu_speciallwturn_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    revenge_pre(fighter, false)
}

unsafe extern "C" fn pikachu_speciallwturn_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let air = fighter.global_table[0x16] != *SITUATION_KIND_GROUND;
    let motion = if air { "special_air_lw_turn" } else { "special_lw_turn" };
    MotionModule::change_motion(boma, Hash40::new(motion), 0.0, 1.0, false, 0.0, false, false);
    GroundModule::correct(
        boma,
        GroundCorrectKind(if air { *GROUND_CORRECT_KIND_AIR } else { *GROUND_CORRECT_KIND_GROUND_CLIFF_STOP }),
    );
    revenge_kinetics(fighter, true);
    crate::telemetry::probe::event(format!(
        "[rat] REVENGE retaliation from BEHIND, motion '{motion}'; {}",
        crate::telemetry::probe::ctx(boma)
    ));
    fighter.sub_shift_status_main(L2CValue::Ptr(pikachu_speciallw_main_loop as *const () as _))
}

unsafe extern "C" fn pikachu_speciallw_main_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);

    // MEASURED BEFORE IT IS RESTORED, because the last two explanations for this were both wrong.
    //
    // I said the autocancel came from `is_enable_cancel` reading Pikachu's own FAF instead of
    // Incineroar's. It does not: `mlshow` on the shipped motion_list and on gaogaen's give the
    // SAME numbers -- `special_lw` faf 27, `special_lw_start` faf 59 -- because
    // `tools/motion-patch` already copies both. There was nothing to patch and the story was
    // invented.
    //
    // So this logs what the cancel WOULD have done, once per stance, and changes nothing. The
    // question it answers is the one that actually matters: on which frame does the engine say
    // the FAF has passed, and is that BEFORE the ACMD's frame-28 `close_window`? If it is, the
    // armed-counter bug is real and restoring the check needs the window closed on exit too. If
    // it is not, the autocancel had some other cause and this says so before another round is
    // spent on it.
    if !CANCEL_LOGGED[entry] && CancelModule::is_enable_cancel(boma) {
        CANCEL_LOGGED[entry] = true;
        crate::telemetry::sink::diag(format!(
            "[rat] REVENGE p{entry} is_enable_cancel went TRUE on status frame {} \
             (motion {:.1}/{:.1}, window_open={}, phase={}) -- the cancel is NOT wired up; this \
             is the measurement for whether it can be",
            fighter.global_table[0xe].get_i32(),
            MotionModule::frame(boma),
            MotionModule::end_frame(boma),
            WINDOW_OPEN[entry],
            PHASE[entry],
        ));
    }

    // RESTORED, ON THE MEASUREMENT RATHER THAN ON A STORY. Two of my explanations for this were
    // wrong; the probe that replaced them answered it in one playtest:
    //
    //   REVENGE p0 is_enable_cancel went TRUE on status frame 49 (motion 58.0/94.0,
    //                                                             window_open=false, phase=0)
    //   REVENGE p0 is_enable_cancel went TRUE on status frame 26 (motion 26.0/67.0,
    //                                                             window_open=false, phase=1)
    //
    // The stance's FAF lands on frame 49 of a NINETY-FOUR frame motion, and without this check
    // the only way out is `is_end` -- so Pikachu was committed for 45 frames past the point
    // Incineroar is free. The playtest confirms it from the other side: "rat of doom's FAF for
    // revenge is clearly later than native incineroar; even visually I can confirm this 100%".
    //
    // AND `window_open=false` IN BOTH CASES, which retires the reason I reverted it. I claimed
    // cancelling before the ACMD's frame-28 `close_window` would leave the counter armed. The FAF
    // arrives long after the window has shut, and `WINDOW_FRAMES` expires it in 26 frames
    // regardless, so that state is not reachable from here.
    //
    // The earlier "too quickly" report is this check working: frame 49 instead of frame 94 IS
    // faster, and against a move that had been 45 frames too long it read as an autocancel.
    if CancelModule::is_enable_cancel(boma) {
        if fighter.sub_wait_ground_check_common(false.into()).get_bool() {
            return 1.into();
        }
        if fighter.sub_air_check_fall_common().get_bool() {
            return 1.into();
        }
    }

    // AND THE ANIMATION FOLLOWS THE SITUATION TOO, WHICH THIS LOOP WAS NOT DOING.
    //
    // The kinetics swap above is right and was only ever half of it. `SpecialLw/main_loop.c`
    // pairs every `GroundModule::correct` with a `MotionModule::change_motion_inherit_frame`, in
    // BOTH directions, so Incineroar landing mid-Revenge switches to the ground animation at the
    // frame he was on -- and taking off switches to the air one. This port kept playing whichever
    // animation the move started in. `check-callparity.sh` is what surfaced it:
    // `change_motion_inherit_frame` is called by all three of his SpecialLw statuses and by
    // nothing here.
    //
    // THE MOTION PAIR IS PER STATUS, and the hashes are the source's own, resolved rather than
    // assumed -- `tools/motion-patch h40` against the literals in the decomp:
    //
    //     SpecialLw     0x10ef97f71f special_lw_start   0x146e22785f special_air_lw_start
    //     SpecialLwHit  0x0ab6928cf2 special_lw         0x0e46c0e666 special_air_lw
    //     SpecialLwTurn 0x0fa4760a42 special_lw_turn    0x13afef8796 special_air_lw_turn
    //
    // The first pair is what the stance plays, the second is the retaliation and the third is the
    // retaliation from behind, which is exactly what this module's three mains choose between on
    // entry. Reading the hashes mattered: `special_lw` looked like the obvious answer for the
    // stance and is the wrong one.
    let now_ground = fighter.global_table[0x16] == *SITUATION_KIND_GROUND;
    let was_ground = fighter.global_table[0x17] == *SITUATION_KIND_GROUND;
    if now_ground != was_ground {
        let motion = match (StatusModule::status_kind(boma), now_ground) {
            (RAT_STATUS_KIND_SPECIAL_LW_HIT, true) => "special_lw",
            (RAT_STATUS_KIND_SPECIAL_LW_HIT, false) => "special_air_lw",
            (RAT_STATUS_KIND_SPECIAL_LW_TURN, true) => "special_lw_turn",
            (RAT_STATUS_KIND_SPECIAL_LW_TURN, false) => "special_air_lw_turn",
            (_, true) => "special_lw_start",
            (_, false) => "special_air_lw_start",
        };
        GroundModule::correct(
            boma,
            GroundCorrectKind(if now_ground {
                *GROUND_CORRECT_KIND_GROUND_CLIFF_STOP
            } else {
                *GROUND_CORRECT_KIND_AIR
            }),
        );
        MotionModule::change_motion_inherit_frame(
            boma, Hash40::new(motion), -1.0, 1.0, 0.0, false, false,
        );
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

/// WHICH SIDE THE HIT CAME FROM, READ WHERE THE ENGINE READS IT.
///
/// `damage_lr_` is a field of `app::DamageInfo` and the only way into it from a plugin is the L2C
/// table the engine hands a CheckDamage line -- `DamageInfo__store_l2c_table_impl` writes offset
/// 0x160 under hash 0xa66ec16de, and no `DamageModule` binding exposes it. So this line exists to
/// read one number at the one moment it is available.
///
/// IT DECIDES NOTHING AND RETURNS `false`. The counter trigger is still the damage delta in
/// `pikachu_revenge_frame`, which the last playtest confirmed; this only records the sign so that
/// trigger can pick between the two retaliation statuses. `false` is what a status with no
/// CheckDamage line effectively answers -- DK's carry line returns `false` on every path where it
/// does not apply, and being hit during his carry in that state behaves like any other status.
///
/// INCINEROAR REGISTERS NO CheckDamage ON SpecialLw, and that is not a contradiction: his branch
/// is in `main`, in compiled C++ that runs on Gaogaen and not on Pikachu. There is no native
/// status line to transcribe here, so this is the port's own route to the same input -- the
/// guards (`damage_add_ > 0`, the stance window) stay where they already work rather than being
/// duplicated.
unsafe extern "C" fn revenge_check_damage(
    fighter: &mut L2CFighterCommon,
    damage_info: &L2CValue,
) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    // UNCONDITIONAL, ONCE PER STANCE, AND IT IS THE TEST. The existing diag below only fires when
    // the window is open AND the damage is positive, so its silence could mean this never ran OR
    // that it ran and both guards were false. Those need different fixes: the first is a broken
    // registration, the second is the port's own trigger. `revenge_turn` was reported not working
    // and this line is what tells the two apart.
    if !CHECK_DMG_RAN[entry] {
        CHECK_DMG_RAN[entry] = true;
        crate::telemetry::sink::diag(format!(
            "[rat] REVENGE p{entry} check_damage RAN (window_open={}) -- the CheckDamage line IS \
             installed on SPECIAL_LW",
            WINDOW_OPEN[entry]
        ));
    }
    if WINDOW_OPEN[entry] {
        // `damage_info["damage_add_"] > 0.0`, the engine's own guard at 0xab9f40 -- a zero-damage
        // connection is not a counter.
        let add = damage_info[DAMAGE_ADD].get_f32();
        if add > 0.0 {
            let lr = damage_info[DAMAGE_LR].get_f32() * PostureModule::lr(boma);
            FROM_BEHIND[entry] = lr < 0.0;
            crate::telemetry::sink::diag(format!(
                "[rat] REVENGE p{entry} check_damage: damage_add_={add:.2} \
                 damage_lr_*lr={lr:.2} -> from_behind={}",
                FROM_BEHIND[entry]
            ));
        }
    }
    false.into()
}

/// `damage_info["damage_add_"]` and `["damage_lr_"]`, the two fields the engine's Revenge branch
/// reads. Literals rather than `hash40(..)` because these are C++ member names exposed into L2C by
/// `DamageInfo__store_l2c_table_impl` -- `tools/luaconst --hashdict` names them off main's own
/// strings and no community label list has them.
const DAMAGE_ADD: u64 = 0xb9ea8580a;
const DAMAGE_LR: u64 = 0xa66ec16de;

unsafe extern "C" fn pikachu_speciallw_end(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);

    // PHASE IS RESET ON EVERY EXIT, NOT ONLY WHEN THE ANIMATION RUNS OUT.
    //
    // "pikachu can now sometimes activate the revenge counter fire damage on command while having
    // a stored revenge boost... the fire damage should only activate if an opponent hits the
    // counter." One stuck variable, and it produced BOTH symptoms this move has had.
    //
    // The stance's own `main` picks its animation from PHASE:
    //
    //     let motion = match (PHASE[entry] == PHASE_COUNTER, air) {
    //         (true,  false) => "special_lw",         // the RETALIATION animation
    //         (false, false) => "special_lw_start",   // the stance animation
    //
    // and `PHASE[entry] = PHASE_STANCE` lived ONLY inside `main_loop`'s `is_end` branch. Restoring
    // the FAF cancel means the loop returns at the FAF and `is_end` is never reached -- so after
    // one real counter PHASE stayed COUNTER for the rest of the match, and every later down-B
    // played the fire.
    //
    // AND THAT IS ALSO THE "TOO QUICKLY" FROM THE ROUND BEFORE. `special_lw` is faf 27;
    // `special_lw_start` is faf 59. Playing the wrong motion moved the FAF 32 frames earlier, so
    // the cancel came up almost at once. Two reports, one cause, and the probe printed the
    // evidence in plain sight: every `is_enable_cancel went TRUE` line in pt34 reads `phase=1`.
    //
    // NOT WHEN THE COUNTER IS WHAT WE ARE LEAVING FOR. `globalTable[0xb]` is the next status, the
    // same idiom the whip's release and DK's carry exit both use. The counter sets PHASE and then
    // requests SPECIAL_LW_HIT -- or SPECIAL_LW_TURN when the hit came from behind -- and clearing
    // it on either transition would undo the thing that just decided to happen.
    let next = fighter.global_table[0xb].get_i32();
    if next != RAT_STATUS_KIND_SPECIAL_LW_HIT && next != RAT_STATUS_KIND_SPECIAL_LW_TURN {
        PHASE[entry] = PHASE_STANCE;
        WINDOW_OPEN[entry] = false;
    }

    revert_absorb(boma);
    0.into()
}

// Runs every frame regardless of status, which is the only place the counter CAN be detected:
// being hit puts the fighter in a damage status, so the SPECIAL_LW status loop stops running at
// exactly the moment the hit lands.
/// Is this one of Revenge's own statuses?
///
/// ONE DEFINITION, USED EVERYWHERE, BECAUSE TWO COPIES DESYNCHRONISED IMMEDIATELY.
///
/// The retaliation used to re-enter SPECIAL_LW, and moving it to its own SPECIAL_LW_HIT status --
/// which is what Incineroar does, and what made the fire hitbox exist at all -- broke a SECOND
/// test that had been written as `status == FIGHTER_STATUS_KIND_SPECIAL_LW` fifty lines away. The
/// log says it plainly:
///
///   REVENGE retaliation status entered, motion 'special_air_lw'; st=513(=0x201)
///   REVENGE p0 boost 2.07 CONSUMED by a landed attack in 513(=0x201)
///
/// `in_counter_attack` went false, so the counter's own fire was treated as an ordinary attack:
/// it took the stored boost AND spent it. Playtest: "revenge is back to spending the bonus damage
/// on the fire counter damage" -- the exact symptom a previous round had fixed, reintroduced by a
/// change that looked unrelated.
///
/// This project has hit that shape repeatedly and named it: a parallel list. So there is no second
/// list. Every "are we in Revenge" question goes through here.
fn is_revenge_status(status: i32) -> bool {
    unsafe {
        status == *FIGHTER_STATUS_KIND_SPECIAL_LW
            || status == RAT_STATUS_KIND_SPECIAL_LW_HIT
            || status == RAT_STATUS_KIND_SPECIAL_LW_TURN
    }
}

/// Catch the absorb calls being left armed outside the two statuses that may arm them.
///
/// A SELF-CHECK FOR A WHOLE CLASS OF BUG, not for one instance of it. `open_window` arms three
/// pieces of ENGINE state -- halved incoming damage, multiplied hitlag, and
/// DAMAGE_NO_REACTION_MODE_ALWAYS -- and if any path out of Revenge forgets to disarm them,
/// Pikachu keeps them. Permanently. Halved damage and total flinch immunity for the rest of the
/// stock, with nothing on screen to say so and nothing in the log either.
///
/// That is not hypothetical: `End` did nothing at all until the decomp was read, and
/// `SpecialLw_end` turns out to be exactly those three reverts. The bug was live and invisible.
///
/// So the invariant is stated once and checked every frame, on every status: armed implies we are
/// in SPECIAL_LW or SPECIAL_LW_HIT. A violation is reverted and reported by name, because the
/// next path that forgets should cost one log line rather than another playtest.
unsafe fn watch_absorb_leak(boma: *mut BattleObjectModuleAccessor, entry: usize, status: i32) {
    if !ABSORB_ARMED[entry] {
        return;
    }
    if is_revenge_status(status) {
        return;
    }
    crate::telemetry::sink::crumb(format!(
        "[rat] REVENGE p{entry} ABSORB LEAK: the three absorb calls were still armed in status \
         {status:#x}, which is neither SPECIAL_LW nor SPECIAL_LW_HIT. Reverting. This means a \
         path out of Revenge did not call revert_absorb -- find it, because until this fired \
         Pikachu was on 0.4x incoming damage with no flinch."
    ));
    revert_absorb(boma);
}

pub unsafe extern "C" fn pikachu_revenge_frame(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let entry = entry_of(boma);
    watch_absorb_leak(boma, entry, StatusModule::status_kind(boma));
    let damage = DamageModule::damage(boma, 0);
    let taken = damage - LAST_DAMAGE[entry];

    // A hit landed while the window was open.
    if WINDOW_OPEN[entry] && WINDOW_LEFT[entry] > 0 && taken > 0.0 {
        // HE KEEPS 40% OF THE DAMAGE. THIS REFUNDED ALL OF IT.
        //
        // `special_lw_hit_damage_mul` is 0.4 and this file read it as "the retaliation's damage
        // scales with what it ate". It does not. SmashWiki, quoted in
        // vendor/smashwiki/revenge.wikitext: "When initially triggered, Incineroar receives 0.4x
        // of the incoming attack's damage". The param is what the COUNTERER TAKES.
        //
        // So the refund is 60%, not 100%. Countering a 15% attack should leave Incineroar 6%
        // worse off, and this was leaving him untouched -- which also made Revenge strictly
        // better than the real move rather than a trade.
        // NO REFUND. `set_damage_mul_2nd(0.4)` in open_window already made the engine scale the
        // incoming damage, so `taken` IS the reduced figure and subtracting anything would
        // double-apply it. What the boost formula needs is the attack's RAW damage -- SmashWiki:
        // "ignoring the 0.4x multiplier" -- so it is recovered by dividing back out.
        let raw = if HIT_DAMAGE_MUL > 0.0 { taken / HIT_DAMAGE_MUL } else { taken };
        COUNTERED[entry] = taken;
        RATE[entry] = next_rate(RATE[entry], raw);
        TIMER[entry] = REVENGE_TIME_FRAMES;
        DAMAGE_SINCE_BOOST[entry] = 0.0;
        WINDOW_OPEN[entry] = false;
        PHASE[entry] = PHASE_COUNTER;
        LAST_DAMAGE[entry] = DamageModule::damage(boma, 0);

        // THE FREEZE IS THE ENGINE'S NOW, AND SO THE DEFERRED TRANSITION IS GONE.
        //
        // `set_hit_stop_mul(1.5, 3, 0.0)` was armed when the window opened, so the hit that just
        // landed is ALREADY producing 1.5x its normal hitlag, on both fighters, computed by the
        // game from that attack's own properties. Round 3 rebuilt Ultimate's hitlag formula in
        // Rust, applied SlowModule by hand, and held the status change back by the result. All
        // three of those go away: the pause happens before this frame's transition is acted on,
        // which is exactly the native order.
        //
        // Incineroar changes status here and lets the engine's hitlag hold the frame. So do we.
        let attacker_entry =
            WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_SUCCEED_ATTACKER_ENTRY_ID);
        crate::telemetry::probe::event(format!(
            "[rat] REVENGE p{entry} COUNTERED raw {raw:.1}% (took {taken:.1}% after the 0.4x) \
             -> rate {:.2}; engine hitlag x{HIT_STOP_MUL} holds the pause (attacker entry \
             {attacker_entry}); {}",
            RATE[entry],
            crate::telemetry::probe::ctx(boma)
        ));
        // A DEDICATED STATUS, BECAUSE REQUESTING THE ONE YOU ARE ALREADY IN IS A NO-OP.
        //
        // This asked for FIGHTER_STATUS_KIND_SPECIAL_LW from inside SPECIAL_LW. The engine drops
        // a transition to the current status, so `main` never ran again, the motion stayed on
        // the stance, and `game_speciallw`s fire hitbox never existed -- "Revenge does not seem
        // to cause the fire counter damage on hit". PHASE_COUNTER was set correctly and had
        // nothing to act on it.
        //
        // Incineroar has SpecialLwHit as a real status for exactly this. So do we now.
        //
        // AND SpecialLwTurn WHEN THE HIT CAME FROM BEHIND, which is the engine's own branch --
        // `damage_info["damage_lr_"] * PostureModule::lr < 0` at 0xab9f70, quoted in full above
        // RAT_STATUS_KIND_SPECIAL_LW_TURN. The sign was captured by `revenge_check_damage` at the
        // moment the damage was applied rather than recomputed here, because `lr` can already have
        // changed by the time the delta is noticed.
        let behind = FROM_BEHIND[entry];
        FROM_BEHIND[entry] = false;
        StatusModule::change_status_request_from_script(
            boma,
            if behind { RAT_STATUS_KIND_SPECIAL_LW_TURN } else { RAT_STATUS_KIND_SPECIAL_LW_HIT },
            false,
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
        // THE AIRBORNE COUNTER PLAYS `special_air_lw`, AND THIS ONLY MATCHED `special_lw`.
        //
        // That is the bug behind "it appears to use the revenge damage boost on the counter fire
        // hit instead of the next hit after that". With the test false, `mul` below took the
        // STORED RATE instead of the retaliation multiplier, so the fire hitbox was scaled by the
        // whole banked boost -- and `is_attack_occur` then consumed it, because consuming is
        // gated on the same flag. Both halves of the reported symptom, from one missing name.
        //
        // Driven off the STATUS now rather than the motion. PHASE_COUNTER plus being in
        // SPECIAL_LW is what "this is the retaliation" means; which of the two motions is playing
        // is a detail of whether Incineroar happened to be in the air, and every time this has
        // been keyed on a motion name it has missed a variant.
        let in_counter_attack = PHASE[entry] == PHASE_COUNTER && is_revenge_status(status);

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
            // A `crate::cues::revenge_impact(fighter, RATE[entry])` used to sit here, commented
            // "the boosted hit landed" -- INSIDE THE LOSS BRANCH, so it fired when the boost was
            // destroyed by damage or a throw, which is the opposite moment. It is gone with the
            // rest of that file's invented effects; see src/cues/mod.rs for what the visual
            // actually is.
            clear_boost(fighter);
            LAST_DAMAGE[entry] = damage;
            return;
        }

        // THE COUNTER FIRE IS NOT BOOSTED AT ALL. This scaled it by something either way.
        //
        // SmashWiki: Revenge "produces a minor burst of flame that deals minimal damage with set
        // knockback. However, this ALSO applies a damage boost to the NEXT attack Incineroar
        // lands". Minimal and set -- the 2.4 in the ACMD is the whole of it, and the boost belongs
        // to the attack after. Playtest, exactly: "it appears to use the revenge damage boost on
        // the counter fire hit instead of the next hit after that."
        //
        // `retaliation_damage_mul` was invented to explain `special_lw_hit_damage_mul`, and that
        // param turns out to mean something else entirely -- the 0.4x damage INCINEROAR TAKES.
        // So the function is gone and the retaliation gets no multiplier of any kind.
        // THE SLOTS ARE INCINEROAR'S OWN, AND THEY WERE NOT BEFORE. This wrote `set_power_up` and
        // `set_reaction_mul`, which are the GENERIC multiplier slots -- the ones rage, items,
        // handicap and every other system also write. Decompiling the native teardown settles it:
        //
        //     app::FighterSpecializer_Gaogaen::revenge_off   main 0xab86f0
        //         [boma+0xa0] vtable[0x270] (1.0f)    = AttackModule::set_power_mul_5th
        //         [boma+0xa0] vtable[0x2b8] (1.0f)    = AttackModule::set_reaction_mul_3rd
        //
        // Revenge lives in power slot 5 and reaction slot 3, and resets both to 1.0 -- so those
        // are the slots, and 1.0 is the identity. Writing the generic slot instead meant this
        // moveset's Revenge and anything else that touched the same slot would overwrite each
        // other in whichever order they ran, with the loser silently losing its multiplier.
        //
        // See also `work/ghidra/main-ab86f.annotated.c`, which is also where the rest of the
        // native teardown is legible: three instance flags cleared, two instance ints and two
        // unexported instance floats reset, one `VisibilityModule::set` model-variant swap (the
        // red colouring, recorded in docs/the-weapons.md as unported), and one effect killed.
        if in_counter_attack {
            set_revenge_muls(boma, entry, 1.0, 1.0);
        } else {
            // AND KNOCKBACK IS NOT THE DAMAGE MULTIPLIER. This set both to the stored rate, so a
            // 3x boost meant 3x knockback -- roughly fifteen times too strong.
            //
            // SmashWiki: "Revenge's damage boost is ignored entirely by affected attacks.
            // Instead ... each kind of attack uses its own knockback multiplier, which scales
            // from 1x to the specified amount, proportionally to how close the damage multiplier
            // is to the maximum of 3x." The per-category ceilings are in the article's table and
            // in KB_* below; 1.2x is the most any move gets.
            set_revenge_muls(
                boma,
                entry,
                RATE[entry],
                knockback_mul(RATE[entry], category_kb_ceiling(boma)),
            );
        }
        aura_on(fighter);
        aura_tick(fighter);
        use crate::telemetry::probe;
        // CONSUMED BY THE NEXT LANDED ATTACK, which is the whole point of the move.
        //
        // Three things are exempt, and all three are the game's rules rather than convenience.
        // The counterattack itself, because the boost is explicitly for the attack AFTER it.
        // And pummel, floor attacks and the edge attack, which SmashWiki lists as "not affected
        // by Revenge, and neither use up the boost nor cause storage loss" -- so landing one must
        // not spend a boost the player is holding for something else.
        // THE EDGE, NOT THE FLAG. `is_attack_occur` is STICKY: it stays true for the rest of the
        // move once anything of ours connects. The log shows exactly what that cost:
        //
        //   REVENGE retaliation status entered, motion 'special_air_lw'; st=513(=0x201) f=0.0/67
        //   REVENGE p0 boost 2.16 CONSUMED by a landed attack in 513(=0x201); f=67.0/67
        //
        // Consumed in the retaliation's own status, on the retaliation's LAST frame. The status
        // test was right and `in_counter_attack` was true for sixty-six of those frames -- but
        // `pikachu_speciallw_main_loop` resets PHASE to PHASE_STANCE at `is_end`, so on frame 67
        // it went false while the fire's own `is_attack_occur` was still latched. Playtest:
        // "revenge still seems to use bonus damage on fire counter".
        //
        // Two changes, because either alone leaves a hole. PHASE is out of the test -- being in
        // SPECIAL_LW or SPECIAL_LW_HIT at all means any attack occurring belongs to Revenge,
        // since the stance has no hitbox and the retaliation's fire is the only one either status
        // produces. And the consumption is charged on the RISING EDGE, so a latched flag from a
        // previous move can never spend a boost banked after it.
        let occur = AttackModule::is_attack_occur(boma);
        let attack_edge = occur && !ATTACK_SEEN[entry];
        ATTACK_SEEN[entry] = occur;
        if !is_revenge_status(status) && !revenge_exempt(boma) && attack_edge {
            probe::event(format!(
                "[rat] REVENGE p{entry} boost {:.2} CONSUMED by a landed attack in {}; {}",
                RATE[entry],
                probe::st(status),
                probe::ctx(boma)
            ));
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
// `retaliation_damage_mul` IS GONE. It existed to scale the counter fire by
// `special_lw_hit_damage_mul`, on the reading that 0.4 meant "the retaliation scales with what it
// ate". SmashWiki says the param is the 0.4x damage INCINEROAR TAKES, and that the fire "deals
// minimal damage with set knockback" -- a flat 2.4, unboosted. A function built entirely on a
// misread constant is worse than no function, so it is deleted rather than corrected.

pub fn install() {
    crate::charaslot::agent("pikachu")
        .status(Pre,  *FIGHTER_STATUS_KIND_SPECIAL_LW, pikachu_speciallw_pre)
        .status(Main, *FIGHTER_STATUS_KIND_SPECIAL_LW, pikachu_speciallw_main)
        .status(End,  *FIGHTER_STATUS_KIND_SPECIAL_LW, pikachu_speciallw_end)
        // `Exit` AND NOT `End` ON THE TWO NEW ONES, WHICH WAS A DEAD REGISTRATION AND A LATENT BUG.
        //
        // `tools/check-statuslines.sh` knows why: smashline hooks End and Exit at two offsets that
        // both land inside `L2CFighterBase::sub_end_status_msc`, and only the vanilla path reaches
        // the End one -- so `End` on a brand-new status kind never runs. SPECIAL_LW_HIT had been
        // registering End, and the check EXEMPTED it on the reasoning that gaogaen's own
        // `SpecialLwHit_end` is a bare `return 0` so the body was redundant anyway.
        //
        // That reasoning is right about Incineroar and wrong about this port. `pikachu_speciallw_end`
        // does not only revert the absorb; it also puts PHASE back to PHASE_STANCE, and the
        // retaliation can be FAF-cancelled at frame 27 of a 67-frame motion, which returns from
        // `main_loop` without reaching the `is_end` branch that is the only other place PHASE is
        // cleared. A cancelled retaliation therefore left PHASE at COUNTER -- which is exactly the
        // "activates the revenge counter fire damage on command" shape that has now been reported
        // twice. The stance's own End covers entering the retaliation; nothing covered leaving it.
        .status(Pre,  RAT_STATUS_KIND_SPECIAL_LW_HIT, pikachu_speciallwhit_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_LW_HIT, pikachu_speciallwhit_main)
        .status(Exit, RAT_STATUS_KIND_SPECIAL_LW_HIT, pikachu_speciallw_end)
        .status(Pre,  RAT_STATUS_KIND_SPECIAL_LW_TURN, pikachu_speciallwturn_pre)
        .status(Main, RAT_STATUS_KIND_SPECIAL_LW_TURN, pikachu_speciallwturn_main)
        .status(Exit, RAT_STATUS_KIND_SPECIAL_LW_TURN, pikachu_speciallw_end)
        // Only on the STANCE. The two retaliation statuses cannot be countered out of, and the
        // engine's branch is gated on `status == FIGHTER_STATUS_KIND_SPECIAL_LW` for the same
        // reason.
        .status(CheckDamage, *FIGHTER_STATUS_KIND_SPECIAL_LW, revenge_check_damage)
        .on_line(Main, pikachu_revenge_frame)
        // Zeroes the once-per-life air-speed latch where the engine zeroes its own instance work.
        // See `AIR_SPEED_Y_SPENT`.
        .on_start(revenge_object_start)
        .install();
}
