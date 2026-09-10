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
// THE EXPERIMENT, in this order, one variable each:
//
//   1. clone OFF, card repaired.  Does Pikachu load?   <-- where we are now
//   2. clone ON,  card repaired.  Does Pikachu load?
//
// If 1 and 2 both load, the clone was always fine and Ptooie is unblocked. If 1 loads and 2
// crashes, the clone is genuinely at fault and the next thing to read is `update_weapon_count`.
// If 1 crashes, neither suspect was right and the zero-byte file was a red herring too.
//
// ---------------------------------------------------------------------------------------------
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
