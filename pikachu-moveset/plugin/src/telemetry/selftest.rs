// Asks the GAME, on the first frame of a Pikachu object, whether everything the mod ships
// actually arrived. Writes the answer to the SD card log.
//
// WHY THIS EXISTS. Every offline check in tools/ verifies the files we BUILD. None of them can see
// what the console LOADS, and the gap between those two has been where the real failures live: a
// zero-byte .nuanmb that passed a directory listing, an attribute file shipped as UTF-16 that
// ARCropolis silently declined to parse, a motion_list entry pointing at an animation that was
// never in config.json. Each of those cost a session to find by describing symptoms out loud.
//
// `MotionModule::is_anim_resource` and `end_frame_from_hash` answer the question directly, per
// motion, without playing anything: is there a resolved animation resource behind this motion
// name, and how long is it. A synthesised motion_list entry whose animation did not ship comes
// back false. So one training-mode load now produces the list of what is broken, by name, instead
// of a human noticing that one move looks wrong.
//
// It reports FAILURES individually and everything else as a count. A 75-line pass is noise nobody
// reads; "75/75 resolved" followed by nothing is a result, and any line at all after it is a bug
// with a name attached.
use {
    smash::{app::{lua_bind::*, BattleObjectModuleAccessor}, lib::{lua_const::*, L2CValue}, lua2cpp::L2CFighterCommon, phx::Hash40},
    super::motions::PORTED_MOTIONS,
    super::sink,
};

/// Which entries have already reported. Indexed by entry id; a self-test per player is redundant.
static mut DONE: [bool; 8] = [false; 8];

/// How many more frames until the re-check, per entry. 0 once it has run.
///
/// WHY A SECOND PASS. The first run reported 48 of 77 resolved, and the round before it reported
/// 77 of 77 off the same files -- and worse, the failing SET differed between two loads in the
/// same session: one had every shoulder_* broken and special_lw fine, the next had the reverse.
/// Identical files cannot fail differently, so one of two things is true: either the animations
/// stream in after the first frame of a fighter's life and the check is simply early, or
/// something drops them for good.
///
/// Those need completely different fixes and the log could not tell them apart, so it now asks
/// twice. A name that clears by the second pass was a timing artefact and the check was lying;
/// a name that survives it is really missing.
static mut RECHECK_IN: [i32; 8] = [0; 8];

/// Frames between the two passes. Two seconds: long enough that any streaming this could race
/// has finished, short enough to be inside the first moments of a training-mode load.
const RECHECK_DELAY: i32 = 120;

/// The lengths a source animation is allowed to have. A motion the game could not resolve reports
/// 0.0, and one that resolved to a stub reports 1.0 or less -- neither is a real animation, and the
/// shortest thing this moveset ships is a nine-frame whiff.
const MIN_PLAUSIBLE_FRAMES: f32 = 2.0;

/// The motions the game cannot resolve right now, as `name(res=..,end=..)` strings.
unsafe fn broken_now(boma: *mut BattleObjectModuleAccessor) -> (usize, Vec<String>) {
    let mut resolved = 0usize;
    let mut broken = Vec::new();
    for name in PORTED_MOTIONS {
        let h = Hash40::new(name);
        let has_resource = MotionModule::is_anim_resource(boma, h);
        let end = MotionModule::end_frame_from_hash(boma, h);
        if has_resource && end >= MIN_PLAUSIBLE_FRAMES {
            resolved += 1;
        } else {
            broken.push(format!("{name}(res={has_resource},end={end:.0})"));
        }
    }
    (resolved, broken)
}

/// The second pass. Called every frame from the telemetry hook; does nothing until it is due.
pub unsafe fn tick(boma: *mut BattleObjectModuleAccessor, entry: usize) {
    if entry >= 8 || RECHECK_IN[entry] <= 0 {
        return;
    }
    RECHECK_IN[entry] -= 1;
    if RECHECK_IN[entry] > 0 {
        return;
    }
    let (resolved, broken) = broken_now(boma);
    sink::diag(format!(
        "[rat] SELFTEST p{entry} RECHECK after {RECHECK_DELAY}f: {resolved}/{} resolved",
        PORTED_MOTIONS.len()
    ));
    for chunk in broken.chunks(6) {
        sink::diag(format!("[rat] SELFTEST p{entry} STILL-UNRESOLVED {}", chunk.join(" ")));
    }
}

pub unsafe fn run(fighter: &mut L2CFighterCommon, boma: *mut BattleObjectModuleAccessor, entry: usize, status_count: i32) {
    if entry >= 8 || DONE[entry] {
        return;
    }
    DONE[entry] = true;
    RECHECK_IN[entry] = RECHECK_DELAY;

    // MEASURED: ALL THREE READ 1.0, AND THAT IS NOT THE 0.96 THE MOD CORRECTS AGAINST.
    //
    // This was added believing `PostureModule::scale` was the fighter `scale` param, so it could
    // confirm the 0.96 that every hitbox radius and top-anchored offset in
    // tools/reach/src/bin/topscale.rs is scaled by. Round 1 says otherwise: scale, base_scale and
    // owner_scale are all exactly 1.0 on a normal Pikachu. They are the RUNTIME multiplier -- the
    // thing Giant and Mini move -- and the `scale` param is applied somewhere this cannot see.
    //
    // So this does not settle the bone-radius question in the backlog, and the note printed
    // alongside says so rather than leaving the next reader to re-derive it. It is still worth
    // logging: a value other than 1.0 would mean something is rescaling him, which would make
    // every hitbox measurement in the project wrong at once.
    // THE COSTUME SLOT, BECAUSE THE MOD ONLY SHIPS c00.
    //
    // Every added animation is declared under `fighter/pikachu/c00` in config.json, so on any
    // other slot they simply are not there and the moveset half-works in a way that looks like a
    // code bug. One number rules that in or out, and it had to be guessed at from the outside
    // twice.
    sink::diag(format!(
        "[rat] SELFTEST p{entry} color=c{:02} scale={:.4} base={:.4} owner={:.4} status_kind_count={status_count:#x} \
         (the mod ships c00 ONLY -- any other colour has no added animations at all; \
         scale is the RUNTIME multiplier and reads 1.0, NOT the 0.96 fighter_param scale; count must be >= 0x1f7)",
        WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_COLOR),
        PostureModule::scale(boma),
        PostureModule::base_scale(boma),
        PostureModule::owner_scale(boma),
    ));

    // DID THE STATUS TABLE ACTUALLY GROW? Asserted, not printed for a human to compare.
    //
    // smashline writes the extended total back to `global_table[0xC]`, so this is the only way to
    // confirm from inside the running game that the new kinds registered. It used to log the
    // number next to a note saying what it should be, which is a comparison nobody makes at the
    // moment it matters -- and if it is ever short, every status past the cut is a transition
    // into nothing: Pikachu in free fall or frozen, with no other symptom.
    let want = crate::specials::status::RAT_STATUS_KIND_NUM;
    if status_count < want {
        report_missing_statuses(fighter, entry);
        sink::diag(format!(
            "[rat] SELFTEST p{entry} STATUS TABLE SHORT: the game reports {status_count:#x} kinds \
             and this build declares up to {want:#x}. Every status at or past {status_count:#x} \
             is a transition into nothing."
        ));
    } else {
        sink::diag(format!(
            "[rat] SELFTEST p{entry} status table ok: {status_count:#x} kinds, need {want:#x}"
        ));
    }

    let (resolved, broken) = broken_now(boma);

    sink::diag(format!(
        "[rat] SELFTEST p{entry} motions {}/{} resolved",
        resolved,
        PORTED_MOTIONS.len()
    ));
    // Chunked, because a single line naming forty broken motions is a line nobody can read and
    // some log viewers truncate. Six per line keeps each one under about 120 characters.
    for chunk in broken.chunks(6) {
        sink::diag(format!("[rat] SELFTEST p{entry} UNRESOLVED {}", chunk.join(" ")));
    }
}

/// WHICH OF THE DECLARED STATUSES THE GAME ACTUALLY HAS, ASKED RATHER THAN DEDUCED.
///
/// `global_table[0xC]` went from 0x206 to 0x202 between two builds -- pt30 through pt35 all say
/// `status table ok: 0x206 kinds, need 0x206` and pt36 says `0x202 ... declares up to 0x207` --
/// so adding ONE status at 0x206 lowered the reported maximum by four. That is not what
/// smashline's arithmetic can do: `install_status_scripts` only ever does
/// `max_new = max_new.max(status.id + 1)` over a Vec that every registration `push`es onto, the
/// costume filter is inert here (no `.costume(` anywhere in this plugin, `Costume::default()` is
/// empty and `has_costume` is therefore false), and `StatusLine::CheckDamage as i32` is 0xB which
/// is exactly the game's `LUA_SCRIPT_STATUS_FUNC_CHECK_DAMAGE`.
///
/// Three readings of that code produced three wrong answers, which is where this project's own
/// rule applies: ship a probe that prints the real state at the real moment instead of a fourth
/// guess. `L2CAgentBase::sv_get_status_func` is the game's own accessor for the table smashline
/// writes with `sv_set_status_func`, so asking it per id says which registrations survived --
/// and a status whose Pre is present but whose Main is not is a different bug from one that is
/// absent entirely.
unsafe fn report_missing_statuses(fighter: &mut L2CFighterCommon, entry: usize) {
    use crate::specials::status::*;
    const DECLARED: &[(&str, i32)] = &[
        ("SPECIAL_HI_CLOSE", RAT_STATUS_KIND_SPECIAL_HI_CLOSE),
        ("SPECIAL_HI_OPEN", RAT_STATUS_KIND_SPECIAL_HI_OPEN),
        ("SPECIAL_S_SHOULDER", RAT_STATUS_KIND_SPECIAL_S_SHOULDER),
        ("SPECIAL_S_LARIAT", RAT_STATUS_KIND_SPECIAL_S_LARIAT),
        ("SPECIAL_S_THROW", RAT_STATUS_KIND_SPECIAL_S_THROW),
        ("SPECIAL_S_END", RAT_STATUS_KIND_SPECIAL_S_END),
        ("SPECIAL_S_FAILURE", RAT_STATUS_KIND_SPECIAL_S_FAILURE),
        ("SHOULDER_START", RAT_STATUS_KIND_SHOULDER_START),
        ("SHOULDER_WAIT", RAT_STATUS_KIND_SHOULDER_WAIT),
        ("SHOULDER_WALK", RAT_STATUS_KIND_SHOULDER_WALK),
        ("SHOULDER_TURN", RAT_STATUS_KIND_SHOULDER_TURN),
        ("SHOULDER_FALL", RAT_STATUS_KIND_SHOULDER_FALL),
        ("SHOULDER_LANDING", RAT_STATUS_KIND_SHOULDER_LANDING),
        ("THROW_F_F", RAT_STATUS_KIND_THROW_F_F),
        ("THROW_F_B", RAT_STATUS_KIND_THROW_F_B),
        ("THROW_F_HI", RAT_STATUS_KIND_THROW_F_HI),
        ("THROW_F_LW", RAT_STATUS_KIND_THROW_F_LW),
        ("SPECIAL_LW_HIT", RAT_STATUS_KIND_SPECIAL_LW_HIT),
        ("SHOULDER_JUMP_SQUAT", RAT_STATUS_KIND_SHOULDER_JUMP_SQUAT),
        ("SHOULDER_JUMP_SQUAT_B", RAT_STATUS_KIND_SHOULDER_JUMP_SQUAT_B),
        ("SHOULDER_JUMP", RAT_STATUS_KIND_SHOULDER_JUMP),
        ("SHOULDER_PASS", RAT_STATUS_KIND_SHOULDER_PASS),
        ("SPECIAL_LW_TURN", RAT_STATUS_KIND_SPECIAL_LW_TURN),
    ];
    let mut absent = Vec::new();
    let mut partial = Vec::new();
    // CHECK_DAMAGE IS ASKED FOR SEPARATELY -- AND THE HYPOTHESIS THAT PUT IT HERE IS DISPROVEN.
    //
    // It was asked because 8a62c76 gave the ten carry statuses their first CheckDamage
    // registration, line 0xB on ids 0x1F7..0x205, all past Pikachu's own NUM of 0x1F0 and all
    // higher than any line this plugin had used before (0..7). The guess was an out-of-bounds
    // write. `sv_set_status_func` says otherwise --
    // `vendor/decomp/common/_ZN7lua2cpp12L2CAgentBase18sv_set_status_func...c`:
    //
    //     begin = this+0x48, end = this+0x50, cap = this+0x58        a std::vector
    //     stride 0xb0 = 22 pointers                                  LUA_SCRIPT_STATUS_FUNC_MAX
    //     if (size <= index) { push_back a zeroed 0xb0 block until size == index+1 }
    //     *(begin + index*0xb0 + line*8) = func
    //
    // It GROWS on demand and reallocates, so (0x205, 0xB) is in bounds by construction and no
    // line index below 0x16 can be out of range. The question is kept anyway because it is now
    // cheap and it distinguishes the remaining cases -- and because a NEGATIVE answer from the
    // running game is worth more than a negative answer from reading a decompilation.
    //
    // THE LEAD THAT REPLACES IT, and it is an ordering one. `STATUS_SCRIPTS[pikachu]` is a Vec in
    // push order, and `specials::install()` runs speciallw BEFORE shoulder, so speciallw's ids
    // come first. 0x202 is exactly `SPECIAL_LW_HIT + 1` -- the last id speciallw registers before
    // SPECIAL_LW_TURN. A list that stopped being walked at that point would report 0x202 and lose
    // shoulder's 0x202..0x205 as collateral, which is precisely pt36. So the shape to look for is
    // the registration list being truncated at the first SPECIAL_LW_TURN entry, not a bad write.
    let mut no_check = Vec::new();
    for (name, id) in DECLARED {
        let pre = has_line(fighter, *id, *LUA_SCRIPT_STATUS_FUNC_STATUS_PRE);
        let main = has_line(fighter, *id, *LUA_SCRIPT_STATUS_FUNC_STATUS_MAIN);
        if *id >= RAT_STATUS_KIND_SHOULDER_START
            && *id <= RAT_STATUS_KIND_SHOULDER_PASS
            && !has_line(fighter, *id, *LUA_SCRIPT_STATUS_FUNC_CHECK_DAMAGE)
        {
            no_check.push(format!("{name}({id:#x})"));
        }
        match (pre, main) {
            (true, true) => {}
            (false, false) => absent.push(format!("{name}({id:#x})")),
            _ => partial.push(format!("{name}({id:#x}) pre={pre} main={main}")),
        }
    }
    if !no_check.is_empty() {
        sink::diag(format!(
            "[rat] SELFTEST p{entry} STATUS PROBE no CheckDamage on: {}",
            no_check.join(" ")
        ));
    }
    if absent.is_empty() && partial.is_empty() {
        sink::diag(format!(
            "[rat] SELFTEST p{entry} STATUS PROBE: every declared status has a Pre AND a Main in \
             the game's own table, so the short count is NOT a missing registration"
        ));
        return;
    }
    if !absent.is_empty() {
        sink::diag(format!(
            "[rat] SELFTEST p{entry} STATUS PROBE ABSENT (no Pre, no Main): {}",
            absent.join(" ")
        ));
    }
    if !partial.is_empty() {
        sink::diag(format!(
            "[rat] SELFTEST p{entry} STATUS PROBE PARTIAL: {}",
            partial.join(" ")
        ));
    }
}

/// `sv_get_status_func` returns a pointer L2CValue, or something that is not a pointer when the
/// slot is empty -- `try_pointer` is the same test smashline's own `set_status_scripts` uses to
/// decide whether an original exists.
unsafe fn has_line(fighter: &mut L2CFighterCommon, id: i32, line: i32) -> bool {
    let id = L2CValue::new_int(id as u64);
    let cond = L2CValue::new_int(line as u64);
    fighter.sv_get_status_func(&id, &cond).try_get_ptr::<()>().map_or(false, |p| !p.is_null())
}
