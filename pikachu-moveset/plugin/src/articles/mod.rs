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
