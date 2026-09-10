// Neutral B — Piranha Plant's Ptooie, whose spiked ball is the `spikeball` article.
//
// Recipe: ssbu-mod-workshop/episode46/ArticleCloningGuide.md.
//
// THE ARTICLE ID WAS WRONG. It was set to 0x7 on the belief that Pikachu has seven vanilla
// articles. He has six, and skyline-smash's own constant table proves it -- his ids run
// DENGEKIDAMA 0x0, DENGEKI 0x1, SPECIALUPDUMMY 0x2, CLOUD 0x3, VORTEX 0x4, MONSTERBALL 0x5.
// So 0x6 is the next free id and 0x7 left a gap. Worth trying 0x6 before anything else when
// this is next attempted; a clone registered into a gap is a plausible cause of the load crash.
//
// The same table is why the ported f-tilt and d-air were writing to Pikachu's own state: article
// and work-flag ids are per-fighter enumerations that collide across fighters. See
// tools/reach/src/bin/deforeign.rs.
//
// The first attempt at this crashed the game on fighter load -- a null dereference on
// TaskWorker2 inside smashline, with `load_fighter_effects + 0x48` as the top frame.
//
// WHAT THE PARAM MACHINERY ACTUALLY DOES, read out of vendor/sl_params.rs rather than inferred.
// `init_fighter_p_object` returns immediately unless the fighter has a registered new article or a
// whitelisted param; otherwise it reads the prc stream the game is ALREADY loading for that
// fighter -- the host's own vl.prc -- builds `allowed_names` (automatically including
// `param_<new_article_name>` for every registered clone), filters the prc to those keys, and
// serves them through a replaced `get_param_object`. So the article's param object really does
// have to be present in the HOST's vl.prc; that part of the old note was right.
//
// TWO THINGS IN THE OLD NOTE WERE WRONG, and both matter:
//
// 1. It claimed the missing param was the crash's root cause. The evidence never supported that.
//    `init_fighter_p_object` and `load_fighter_effects` are different functions, and the crash was
//    in the latter -- which handles effect search indices and never touches params.
// 2. `smashline::add_param_object(fighter, object)` exists and was never tried. It feeds the same
//    WHITELISTED_PARAMS the filter reads. For a CLONE it is redundant (the article registration
//    already adds the name), but it is the supported way to whitelist a param object without
//    editing a vl.prc at all.
//
// AND SHIPPING A MODIFIED vl.prc IS NOW ITSELF SUSPECT. v0.24 shipped one -- Incineroar's
// param_special_s, nothing to do with articles -- and crashed on fighter load in
// load_fighter_effects, the same function. Removing it fixed the crash. But that deploy ALSO had a
// failed upload and a zero-byte .nuanmb on the card, and those files are declared in config.json's
// new-dir-files, which injects them into the search filesystem that load_fighter_effects indexes
// into. Two candidates, one data point each. tools/deploy.sh now repairs truncated uploads
// automatically, which removes the second candidate from future runs and makes the next attempt
// actually informative.
//
// tools/prc-tool/src/bin/vlmerge.rs still exists to copy the param object in, and tools/check.sh
// step 3b enforces that it and CLONE_ENABLED agree in both directions.
//
// Visual note: the ball's appearance comes from its MODEL files, which are shipped, not from
// effects. Absent packun's effect archive the particle work will not play, but the ball itself
// should still have a physical form -- which is the requirement.
use smashline::*;

// Whether the article clone is actually installed. Declared rather than inferred: the call below
// is commented out, and tools/check.sh grepping for the call itself matched the comment and
// demanded the vl.prc override that the disabled state must NOT ship.
//
// While this is false, Pikachu must keep his vanilla vl.prc. `param_spikeball` would declare a
// 21st article param with no weapon behind it, and Training Modpack's save state walks article
// indices calling get_article and remove_exist_object_id.
pub const CLONE_ENABLED: bool = false;

pub const FIGHTER_PIKACHU_GENERATE_ARTICLE_SPIKEBALL: i32 = 0x6;

pub fn install() {
    // Gated rather than commented out, so enabling this is a one-line change and the call itself
    // stays compiled and type-checked. TO TRY IT, three steps in order:
    //
    //   1. set CLONE_ENABLED = true above
    //   2. ship Pikachu's vl.prc with param_spikeball merged in
    //      (tools/prc-tool/src/bin/vlmerge.rs) -- check.sh step 3b enforces this pairing in both
    //      directions and will refuse to deploy if only one of the two is done
    //   3. port Piranha Plant's neutral-B ACMD, which is deliberately NOT ported yet: without a
    //      working article it would spawn nothing, making neutral-B strictly worse than the
    //      Thunder Jolt it replaces
    //
    // Do it AFTER testing everything else. The previous attempt crashed on fighter LOAD, which
    // blocks testing every other move rather than just this one.
    if CLONE_ENABLED {
        smashline::clone_weapon(
            "packun",
            *smash::lib::lua_const::WEAPON_KIND_PACKUN_SPIKEBALL,
            "pikachu",
            "spikeball",
            true,
        );
    }
}
