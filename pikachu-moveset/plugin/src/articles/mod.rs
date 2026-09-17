// Neutral B -- Piranha Plant's Ptooie, whose spiked ball is the `spikeball` article.
//
// Recipe: vendor/ssbu-mod-workshop/episode46/ArticleCloningGuide.md.
//
// ============================================================================================
// THE CLONE MAY NEVER HAVE BEEN THE PROBLEM. Read this before re-litigating the crash.
// ============================================================================================
//
// Two load crashes were blamed on the clone. Both reports are byte-identical -- Data Abort, fault
// address 0, TaskWorker2, and `ReturnAddress[06]: smashline + 0x2a68c
// (fighter_initialize_object_data + 0x2c)`. That hook (vendor/sl_mod.rs:11) only sets
// CURRENT_PLAYER_ID / CURRENT_OWNER_KIND and calls the original, so the null deref is the GAME
// initialising something, not smashline mishandling it.
//
// What was NOT known at the time: the card was also holding a ZERO-BYTE
// `motion/body/c00/d03speciallwstart.nuanmb`. It arrived truncated with the Revenge-air port, the
// then-current verifier checked presence via a directory LISTING -- which sys-ftpd truncates under
// load, so the name being there proved nothing -- and it stayed at zero bytes across every deploy
// after that, including both clone crashes. The size-based verifier found it on its first run and
// repaired it.
//
// A zero-byte .nuanmb declared in config.json is at least as good an explanation for a null deref
// during fighter object init as a cloned article is, and unlike the clone it is CONFIRMED to have
// been present. So the clone is no longer the leading suspect; it is one of two. The earlier
// conclusion that "the asset hypothesis is disproven" was checking the wrong asset: the spikeball's
// own files were indeed on the card, but an animation belonging to a completely different move was
// not.
//
// THE EXPERIMENT HAS RUN, AND THE CLONE IS AT FAULT.
//
//   1. clone OFF, card repaired.  Does Pikachu load?   YES -- 42 of 42 (work/pt2..pt40.log)
//   2. clone ON,  card repaired.  Does Pikachu load?   NO  -- crash, work/crash-0917b.log
//
// Step 2 was recorded as PASSED here and in docs/the-weapons.md on the strength of
// `clone_weapon returned index 0`. That line only proves the call installed; it is logged before
// the fighter is ever built. Segmenting the telemetry by boot, a fighter object was alive 0 times
// across ten clone-on boots and 42 of 42 clone-off. Nine of those ten predate the `share-to-added`
// fix and so prove nothing either way -- this tenth one is the clean run.
//
// Data Abort, Address 0x138, Fault Address 0, thread TaskWorker2, with
// `ReturnAddress[06]: smashline + 0x2a68c (fighter_initialize_object_data + 0x2c)` and every
// other frame in the game's own weapon-creation path. Identical to the two September crashes that
// were blamed on the clone and then excused -- except that all three excuses are now gone: the
// zero-byte d03speciallwstart.nuanmb is repaired, `share-to-added` is fixed, and the spikeball's
// 24 assets are shipped AND declared. So gen-article-assets.sh's header is wrong where it says
// the missing assets explain the crash. They are present, and it still crashes.
//
// `update_weapon_count` was named here as the next thing to read and IS NOT THE ANSWER.
// vendor/sl_weapons.rs has two consumers: :145 runs while `new_descriptors` still holds only the
// fighter's original articles -- ours is pushed afterwards at :153 -- so it cannot reach a cloned
// article's max_count, and :378 does run late enough but lives in the Kirby-copy function.
//
// Two more readings ruled out, both from vendor source with no console:
//   * the `original_article_id` argument is NOT wrong. sl_api.rs:325 indexes
//     LOWERCASE_WEAPON_NAMES with it, and that table is 0x267 = 615 long while WEAPON_KIND_*
//     runs 0..615 -- it is keyed by weapon KIND, so WEAPON_KIND_PACKUN_SPIKEBALL (0x1E8) is right.
//   * NEW_AGENTS is NOT empty. sl_api.rs:358 pushes unconditionally; `use_original_code` is
//     stored on the agent, not a condition for creating it. The weapon_*_hook name rewrites have
//     their data.
//
// WHAT THE FAULT ADDRESS NARROWS IT TO. 0x138 off a null base is a read of `obj->field_0x138` on
// an absent OBJECT, not a jump through a bad function pointer. The game asked for the weapon
// instance behind the appended descriptor, got null, and read a field off it. The known hole that
// fits is gen-article-assets.sh's own: a cloned article resolves under the OWNER's costume slot,
// and vanilla has no fighter/pikachu/spikeball at ANY slot to fall back to -- only c00 ships.
//
// NEXT, AND IT NEEDS NO CONSOLE: the frames above [06] are all `main`, with module-relative
// offsets 0x33a07bc, 0x3a640c, 0x3af458, 0x6875c4, 0x3a761c, 0x6081fc. `tools/decomp.sh main`
// reads them and shows what returns null. See docs/the-weapons.md and docs/the-ptooie.md.
//
// ---------------------------------------------------------------------------------------------
//
//
// WHAT THE PARAM MACHINERY ACTUALLY DOES, read out of vendor/sl_params.rs rather than inferred.
// `get_param_reimpl` FALLS THROUGH to `original_lookup_param` on a miss, so a missing
// `param_spikeball` does not crash -- the ball would run on fall-through params. No vl.prc override
// is shipped or needed, which keeps the experiment above to one variable.
// (`smashline::add_param_object` is the supported way to whitelist a param object without touching
// a vl.prc at all, if those numbers ever turn out to matter.)
//
// WHY THE CLONE IS WORTH THE RESTARTS IT COSTS. `use_original_code: true` makes smashline load
// Piranha Plant's fighter module and run the spikeball's OWN code (get_weapon_code_dependency in
// vendor/sl_create_agent.rs) -- the arc, the juggling, the damage, all exactly right. The
// alternative is hand-writing a projectile's whole lifecycle onto one of Pikachu's own articles
// (DENGEKIDAMA 0x0 is free, since Thunder Jolt is the move being replaced), which is an order of
// magnitude more work and still looks like an electro ball.
//
// ASSETS ARE SHIPPED BY tools/gen-article-assets.sh, gated on CLONE_ENABLED, and
// tools/gen-config.sh derives the declarations from whatever that leaves in the tree -- so
// "declared but absent" and "shipped but undeclared" are both unreachable states now.
use smashline::*;

// Whether the article clone is actually installed. A declared constant rather than a commented-out
// call, because tools/check.sh greps for the call and a comment matched it -- and because flipping
// one `bool` is the whole of running step 2 of the experiment above.
//
// tools/check.sh step 3b enforces that this and the shipped vl.prc agree in BOTH directions, and
// tools/gen-article-assets.sh ships or removes the ball's model and motion to match. So the only
// manual part of enabling this is porting Piranha Plant's neutral-B ACMD, which is deliberately
// still absent: without a working article it would spawn nothing, making neutral-B strictly worse
// than the Thunder Jolt it replaces.
pub const CLONE_ENABLED: bool = false;

pub const FIGHTER_PIKACHU_GENERATE_ARTICLE_SPIKEBALL: i32 = 0x6;

pub fn install() {
    // The numbers get logged whether or not the clone is on, because "what is the spikeball's
    // weapon kind on this build" turned out to be a question nobody had ever written down, and it
    // is free to answer here. It also puts a line in the log BEFORE the risky call, so the log
    // itself says whether install() reached the clone -- which is the point of sink::diag.
    //
    // NO vl.prc IS NEEDED to turn this on. The old three-step recipe here demanded one; the header
    // explains why that was wrong (get_param_reimpl falls through on a miss). Enabling this is
    // CLONE_ENABLED = true, `bash tools/gen-article-assets.sh`, `bash tools/gen-config.sh`,
    // rebuild, deploy -- all of which tools/check.sh verifies agree.
    let kind = *smash::lib::lua_const::WEAPON_KIND_PACKUN_SPIKEBALL;
    crate::telemetry::sink::diag(format!(
        "[rat] articles: CLONE_ENABLED={CLONE_ENABLED} WEAPON_KIND_PACKUN_SPIKEBALL={kind:#x} \
         expected_article_id={FIGHTER_PIKACHU_GENERATE_ARTICLE_SPIKEBALL:#x}"
    ));

    if CLONE_ENABLED {
        // THE RETURN VALUE WAS BEING DISCARDED, through two crashes. smashline_clone_weapon
        // returns the article's index within the fighter's NEW_ARTICLES list -- so 0 for the first
        // clone, and the real article id is Pikachu's vanilla article count plus that. It also
        // returns early with an existing index if the same clone was registered twice. Logging it
        // turns "what id did this actually get" from an assumption into a fact.
        let idx = smashline::clone_weapon(
            "packun",
            kind,
            "pikachu",
            "spikeball",
            true,
        );
        crate::telemetry::sink::diag(format!(
            "[rat] articles: clone_weapon returned index {idx} -- article id should be 0x6 \
             (Pikachu's six vanilla articles are 0x0..0x5)"
        ));
    }
}
