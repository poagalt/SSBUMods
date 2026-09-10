// Neutral B — Piranha Plant's Ptooie, whose spiked ball is the `spikeball` article.
//
// Recipe: ssbu-mod-workshop/episode46/ArticleCloningGuide.md. Pikachu has exactly seven
// vanilla articles per ultimate-research/param-labels articles/Labels.txt, so 0x7 is free.
//
// The first attempt at this crashed the game on fighter load -- a null dereference on
// TaskWorker2 inside smashline (fighter_initialize_object_data / load_fighter_effects).
//
// Root cause was NOT missing effects, despite appearances. smashline's params.rs
// (init_fighter_p_object) builds its allowed-name list as `param_<new_article_name>` and reads
// it from the HOST fighter's own vl.prc. `param_spikeball` (0xf679aaf13) exists in packun's
// vl.prc and was absent from Pikachu's, so the lookup found nothing. Every one of Pikachu's own
// seven articles has its `param_*` entry; the cloned one had none.
//
// Fix: tools/prc-tool/src/bin/vlmerge.rs copies that one param object into Pikachu's vl.prc,
// which the mod now ships. No effect archive is involved, so this stays CSK-free.
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

pub const FIGHTER_PIKACHU_GENERATE_ARTICLE_SPIKEBALL: i32 = 0x7;

pub fn install() {
    // BISECT STEP A: clone disabled to test whether it or the merged model is the crash.
    /*
    smashline::clone_weapon(
        "packun",
        *smash::lib::lua_const::WEAPON_KIND_PACKUN_SPIKEBALL,
        "pikachu",
        "spikeball",
        true,
    );
    */
}
