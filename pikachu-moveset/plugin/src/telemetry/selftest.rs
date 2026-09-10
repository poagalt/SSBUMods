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
    smash::{app::{lua_bind::*, BattleObjectModuleAccessor}, phx::Hash40},
    super::motions::PORTED_MOTIONS,
    super::sink,
};

/// Which entries have already reported. Indexed by entry id; a self-test per player is redundant.
static mut DONE: [bool; 8] = [false; 8];

/// The lengths a source animation is allowed to have. A motion the game could not resolve reports
/// 0.0, and one that resolved to a stub reports 1.0 or less -- neither is a real animation, and the
/// shortest thing this moveset ships is a nine-frame whiff.
const MIN_PLAUSIBLE_FRAMES: f32 = 2.0;

pub unsafe fn run(boma: *mut BattleObjectModuleAccessor, entry: usize, status_count: i32) {
    if entry >= 8 || DONE[entry] {
        return;
    }
    DONE[entry] = true;

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
    sink::diag(format!(
        "[rat] SELFTEST p{entry} scale={:.4} base={:.4} owner={:.4} status_kind_count={status_count:#x} \
         (scale is the RUNTIME multiplier and reads 1.0 -- NOT the 0.96 fighter_param scale; count must be >= 0x1f7)",
        PostureModule::scale(boma),
        PostureModule::base_scale(boma),
        PostureModule::owner_scale(boma),
    ));

    let mut resolved = 0usize;
    let mut broken: Vec<String> = Vec::new();
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
