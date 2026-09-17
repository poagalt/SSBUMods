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
    smash::{app::{lua_bind::*, BattleObjectModuleAccessor}, lib::lua_const::*, phx::Hash40},
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

pub unsafe fn run(boma: *mut BattleObjectModuleAccessor, entry: usize, status_count: i32) {
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
    // THE COSTUME SLOT, WHICH NO LONGER DECIDES WHETHER THE MOVESET WORKS.
    //
    // This used to read "the mod ships c00 ONLY -- any other colour has no added animations at
    // all", and it was true and worth announcing: every added animation was declared under
    // `fighter/pikachu/c00`, so on any other slot the moveset half-worked in a way that looked
    // exactly like a code bug, and it had to be guessed at from the outside twice.
    //
    // `tools/mirror-costumes.sh` ships the animations for all eight now and `gen-config.sh`
    // declares each under its OWN costume key, so the moves work on every colour. What is still
    // c00-only is the MODEL -- the grafted weapons -- because each costume's body model carries
    // its own textures and mirroring c00's would turn every Pikachu default yellow. So the number
    // still matters, for a narrower reason: on a non-c00 slot the moves are right and Chrom's jab
    // swings no sword.
    sink::diag(format!(
        "[rat] SELFTEST p{entry} color=c{:02} scale={:.4} base={:.4} owner={:.4} status_kind_count={status_count:#x} \
         (animations ship for all 8 colours; the grafted WEAPONS are c00-only, so a non-c00 slot \
         plays every move correctly with no sword visible; \
         scale is the RUNTIME multiplier and reads 1.0, NOT the 0.96 fighter_param scale)",
        WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_COLOR),
        PostureModule::scale(boma),
        PostureModule::base_scale(boma),
        PostureModule::owner_scale(boma),
    ));

    // `global_table[0xC]` IS REPORTED AND NO LONGER ASSERTED ON, BECAUSE IT DOES NOT MEAN WHAT
    // THIS CHECK CLAIMED IT MEANT.
    //
    // The claim was: smashline writes the extended status total here, so a value below
    // RAT_STATUS_KIND_NUM proves the new kinds did not register, and "every status at or past the
    // cut is a transition into nothing". It fired -- 0x202 against a declared 0x207 -- and it sent
    // a whole round of work at a registration bug that does not exist.
    //
    // pt37 refutes it in its own log. The count says 0x202, and the same log contains
    //
    //     5 x  RAT_SHOULDER_WALK(0x1f9)         -> RAT_SHOULDER_JUMP_SQUAT_B(0x203)
    //     6 x  RAT_SHOULDER_JUMP_SQUAT_B(0x203) -> RAT_SHOULDER_JUMP(0x204)
    //     3 x  RAT_SHOULDER_JUMP(0x204)         -> RAT_THROW_F_HI(0x1ff)
    //
    // Statuses past the "cut" dispatching, and dispatching through their own Main handlers --
    // those transitions are what `shoulder_jump_main` requests. So the count and the reality are
    // independent, and an assertion tying them together is a false alarm generator.
    //
    // `report_missing_statuses` is gone for the same reason: it asked `sv_get_status_func` per id
    // and reported 0x203 and 0x204 ABSENT in that same log. The contradiction is worth recording
    // rather than explaining away -- `sv_get_status_func` bounds-checks against the vector at
    // `this+0x48` and returns nil past its size, so that vector really did hold 0x202 entries
    // while the engine dispatched 0x204. Something other than that table is dispatching, or it is
    // not the table smashline filled. Unknown, and not guessed at again here.
    //
    // WHAT REPLACED IT is instrumentation inside the functions whose registration was in doubt:
    // `shoulder_check_damage` and `revenge_check_damage` each log once, unconditionally, the first
    // time they run. A function that runs cannot be unregistered, and that evidence cannot be
    // contradicted by a table read.
    let want = crate::specials::status::RAT_STATUS_KIND_NUM;
    sink::diag(format!(
        "[rat] SELFTEST p{entry} global_table[0xC]={status_count:#x}, declared up to {want:#x} \
         -- REPORTED, NOT ASSERTED: pt37 has statuses past this count dispatching normally, so a \
         low value here is not evidence that anything is missing"
    ));

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


