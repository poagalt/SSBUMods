// Real articles, via `smashline::clone_weapon`.
//
// ============================================================================================
// WHY THE VISUALS ARE ARTICLES NOW AND NOT MERGED MESHES
// ============================================================================================
//
// `docs/the-article-path.md` has both halves of the derivation. The short version:
//
// PART 1 -- the clone crash was never the clone. `WeaponSpecializer_PackunSpikeball`'s vtable slot
// 3 (`main 0x3476e20`) reads `FighterParamAccessor2::instance_->fighter_param[FIGHTER_KIND_PACKUN]`
// with the fighter kind CONSTANT-FOLDED IN, and that entry is null in a match with no Piranha
// Plant. Auditing all 114 vtable slots of every article this port wants says five of seven never
// touch their donor's entry at all. `tools/check-article-donor-params.sh` re-derives that from the
// dumps and gates the `clean` column below, with the spikeball and Chrom's sword as controls.
//
// PART 2 -- the mesh graft cannot be fixed, because it is already correct. Nine candidate causes
// of the facing bug were eliminated by measurement (both Modl files complete, culling spheres
// exact, entry flags uniform, billboards disabled, xmb per-model not per-mesh, winding consistent,
// nuhlpb touching only `H_*` bones, bone limit irrelevant). Facing is `rot_y_lr` -- a RIGID Y
// ROTATION -- so it cannot move one mesh relative to the others. And the shipped `chrom_sword`
// vertices equal `source . local(Sword)` to 0.000001 over 444 vertices, which is the native
// attachment exactly. That build was deployed, verified byte-identical on the card, and played:
// still wrong facing left.
//
// So the geometry is right and the bug is somewhere the data cannot see. The answer is to stop
// owning the problem. Every article's ROOT BONE IS ITS ATTACHMENT SOCKET --
//
//     chrom/model/sword          HaveR -> Sword
//     cloud/model/fusionsword    HaveR -> fusionsword1..6
//     elight/model/esword        Root -> HaveR -> Sword1 -> Sword_lightA..F2
//     gamewatch/model/parachute  Have -> Rot -> All -> Parachutea1/a2, b1/b2
//     kamui/model/dragonhand     Attach -> Hand1..4, HandD, HandU
//
// -- so the engine aligns the weapon model's own `HaveR` to the fighter's `HaveR`, and owns the
// attachment, the facing, the culling and the article's own `model.nuanmb`. Three of the five
// weapons this port shows ARE these articles, and all three audit clean.
//
// WHICH TWO ARE NOT, AND WHAT WAS DONE ABOUT IT: Corrin's Yato is
// `Kamui_Weapon_VIS_O_OBJShape` inside `kamui/model/body`, and Palutena's staff is
// `staff_group_VIS_O_OBJShape` inside `palutena/model/body`. Neither is an article in the game and
// neither has a `WEAPON_KIND_*`. So the article they never had is AUTHORED -- two bones,
// socket(identity) plus the bind bone carrying the donor's own local transform, vertices copied
// across untouched -- by `tools/author-article.py`, and hosted on a weapon kind that already
// exists. Both hosts are script-less and audit clean, so they contribute a working article and no
// behaviour. The merged mesh stays as the fallback for each.
//
// ATTACHMENT IS `ArticleModule::have`, found after seven eliminations: it is a call the OWNER
// makes, so for Chrom it lives in his fighter code, which this port does not run. Measured: the
// probe's lateral offset for the parachute went from 4.647 to 0.000 in both facings the moment it
// was called, and Cloud's back-air went from broken to correct both ways.
//
// BOTH EXPERIMENTS THE OLD HEADER PROPOSED WERE NO-OPS, and neither needed a boot:
//   * `add_param_object` -- `sl_params.rs:313` is gated on `WHITELISTED_PARAMS[f] ||
//     NEW_ARTICLES[f]`, and a registered clone already satisfies the second disjunct.
//   * `use_original_code: false` -- the field is stored at `sl_api.rs:369` and READ NOWHERE.
//     The donor module is loaded by `get_weapon_code_dependency(object.kind)`, unconditionally.
use smashline::*;

/// One article that can be cloned onto Pikachu.
///
/// `clean` is a claim about the GAME, not about this mod: false means some function in that weapon
/// class's vtable reads its donor fighter's `FighterParamAccessor2` entry without first checking
/// the owner's kind, which is a null dereference here. `tools/check-article-donor-params.sh`
/// verifies it, so a wrong value is a failed check rather than a surprise on the console.
///
/// `enabled` is ours. Flipping one row off is how this gets bisected if a boot fails, which is why
/// it is per-row and not one global target.
pub struct Candidate {
    /// The article's name under Pikachu: `fighter/pikachu/{model,motion}/<host_name>/cNN`.
    pub host_name: &'static str,
    pub donor: &'static str,
    pub donor_article: &'static str,
    pub weapon_kind: &'static str,
    pub clean: bool,
    pub enabled: bool,
    /// Whether this article's MODEL is built by tools/author-article.py instead of copied from the
    /// donor. True for the two weapons the game does not ship as articles at all -- Palutena's
    /// staff and Corrin's Yato, both meshes inside their owner's body model. For those the `donor`
    /// above is the HOST: a clean, script-less held weapon whose kind and motion files we borrow
    /// and whose geometry we replace.
    pub authored: bool,
    /// Where this article's MODEL comes from, when it is not the donor's own.
    ///
    /// `""` means the donor's, which is every row but one. `"samus/supermissile"` means "copy
    /// that directory into `fighter/pikachu/model/<host_name>/` instead", which is what Samus's
    /// charge shot needs: her `cshot` HAS NO MODEL ANYWHERE IN THE ARCHIVE -- the visible ball is
    /// made entirely of her effects, which ADR 0002 excludes -- so cloning it faithfully produces
    /// a projectile that deals damage and cannot be seen.
    ///
    /// PIKACHU'S ELECTRO BALL WAS TRIED FIRST AND IS A STUB. `fighter/pikachu/model/dengekidama`
    /// has eleven paths in the archive and every one is an empty shell: a 40-byte `model.numatb`
    /// with ZERO material entries, a 144-byte `model.numdlb` with ZERO mesh-to-material rows. His
    /// jolt is a pure-effect article too. Shipping that as a weapon's model gave the game an empty
    /// mesh to build and it stalled on the training-arena load. Her SUPERMISSILE is real -- a
    /// 24,720-byte `model.numshb`, a populated `numatb`, its own `numshexb` bounding spheres and
    /// four textures -- and it is hers, so a charge shot wearing it is at least her own hardware.
    ///
    /// A SUBSTITUTED MODEL IS GEOMETRY ONLY, AND THAT IS WHY IT IS SAFE. The weapon is still
    /// `WEAPON_KIND_SAMUS_CSHOT`, so its physics, its hitboxes and its straight-line flight are
    /// hers; only the mesh is borrowed. The two motions her `motion_list.bin` declares -- `charge`
    /// and `shoot` -- name no animation files at all, so nothing in the lent model has to match
    /// anything in her motion list.
    ///
    /// DECLARED RATHER THAN DONE QUIETLY IN THE GENERATOR, because it is the one place where what
    /// the player sees is not what the game shipped, and because tools/check-articlefiles.sh has
    /// to know to count this article's files against the LENT directory rather than the donor's.
    pub model_from: &'static str,
}

// ARTICLE TABLE. tools/check-article-donor-params.sh and tools/gen-article-assets.sh both parse
// these rows, so the shape is load-bearing -- keep one row per line in this field order.
//
// ORDERED BY RISK, cheapest first, because the article id is the row's position among the enabled
// ones and a crash report names the article that was being built. `sword` and `esword` have NO
// AGENT SCRIPTS AT ALL -- `vendor/SSBU-Dumped-Scripts/smashline/lua2cpp_chrom` contains only
// `chrom`, and `lua2cpp_elight` has no `elight_esword` -- so they are pure models with nothing to
// run. `fusionsword` and `parachute` do have their own agents.
pub const CANDIDATES: &[Candidate] = &[
    // DISABLED 2026-09-19 AS THE FIRST BISECT STEP, and it is the best-motivated single suspect
    // in the table rather than an arbitrary starting point.
    //
    // `CLONE_ENABLED=false` loaded and played a full 3-stock match, so the article system is the
    // cause of the four failed boots. The crash report narrows it further: a null dereference
    // reached from the engine's article TEARDOWN (`RemoveArticleByIndex`) during the DEMO/entry
    // status, where the faulting code is
    //
    //     mov x20, xzr        <- a "not found" branch sets the pointer to NULL
    //     ldr x8, [x20]       <- FAULT
    //     ldr x8, [x8, #0x10]
    //     blr x8              <- a virtual call on a null object
    //
    // That is the engine asking for an article, getting nothing, and calling a method on it
    // anyway. So the thing to look for is an article that is IN THE TABLE but never materialises
    // -- and this row is the only one with a paper trail saying exactly that: `exist=false` on 32
    // consecutive samples across two boots, while `esword`, `fusionsword` and `parachute` all came
    // up on frame 1 or 2.
    //
    // The honest caveat: this row has been enabled for many boots that loaded fine, so "it is
    // null" cannot be the whole story -- something must also have changed about WHEN the engine
    // walks that table, and that part is unknown. If disabling it does not help, the remaining
    // five bisect from the bottom of the table up.
    Candidate { host_name: "sword",       donor: "chrom",     donor_article: "sword",       weapon_kind: "WEAPON_KIND_CHROM_SWORD",         clean: true,  enabled: false, authored: false, model_from: "" },
    // DISABLED 2026-09-19 IN FAVOUR OF THE DUAL-WIELDED MERGED MESH, which is now the proven path.
    //
    // The playtest that settled it: "dual-wielding with chrom works". Chrom's row is off, so his
    // jab falls back to the merged mesh -- and with `tools/dual-wield.py` putting a copy on
    // `HaveL` as well as `HaveR`, that fallback looks right in both facings. The article route
    // never got there: `sword` crashed the game outright, and these two render but wrong.
    //
    // So the article is the worse of two working-ish options for every weapon that HAS a merged
    // mesh, and the fallback is free -- `article_id` returns None for a disabled row and
    // `src/sword/mod.rs` shows the mesh pair instead, with no other change.
    //
    // `fusionsword` stays ON because it is the one article a playtest called fully working in both
    // directions, and `parachute` stays because it has no mesh to fall back to -- it never had one
    // (the canopy is four animated bones, which a rigid graft cannot show).
    // Mythra's dash attack: reported unchanged, i.e. still wrong facing left.
    Candidate { host_name: "esword",      donor: "elight",    donor_article: "esword",      weapon_kind: "WEAPON_KIND_ELIGHT_ESWORD",       clean: true,  enabled: false,  authored: false, model_from: "" },
    Candidate { host_name: "fusionsword", donor: "cloud",     donor_article: "fusionsword", weapon_kind: "WEAPON_KIND_CLOUD_FUSIONSWORD",   clean: true,  enabled: true,  authored: false, model_from: "" },
    Candidate { host_name: "parachute",   donor: "gamewatch", donor_article: "parachute",   weapon_kind: "WEAPON_KIND_GAMEWATCH_PARACHUTE", clean: true,  enabled: true,  authored: false, model_from: "" },
    // AUTHORED. Neither of these is an article in the game, so there is nothing to clone: the
    // model is built from the donor's body model by tools/author-article.py and hosted on a weapon
    // kind that already exists. `ganon/sword` and `pickel/sword` are both script-less
    // (`lua2cpp_ganon` has no `ganon_sword`, `lua2cpp_pickel` no `pickel_sword`) and both audit
    // CLEAN across all 114 vtable slots, so the host contributes a working article and no
    // behaviour at all. The `donor` column is the HOST, which is what the audit and the motion
    // copy both need.
    // DISABLED 2026-09-19 IN FAVOUR OF THE DUAL-WIELDED MERGED MESH, which is now the proven path.
    //
    // The playtest that settled it: "dual-wielding with chrom works". Chrom's row is off, so his
    // jab falls back to the merged mesh -- and with `tools/dual-wield.py` putting a copy on
    // `HaveL` as well as `HaveR`, that fallback looks right in both facings. The article route
    // never got there: `sword` crashed the game outright, and these two render but wrong.
    //
    // So the article is the worse of two working-ish options for every weapon that HAS a merged
    // mesh, and the fallback is free -- `article_id` returns None for a disabled row and
    // `src/sword/mod.rs` shows the mesh pair instead, with no other change.
    //
    // `fusionsword` stays ON because it is the one article a playtest called fully working in both
    // directions, and `parachute` stays because it has no mesh to fall back to -- it never had one
    // (the canopy is four animated bones, which a rigid graft cannot show).
    // Palutena's n-air and grab: never once confirmed correct on screen.
    Candidate { host_name: "staff",       donor: "ganon",     donor_article: "sword",       weapon_kind: "WEAPON_KIND_GANON_SWORD",         clean: true,  enabled: false,  authored: true,  model_from: "" },
    // DISABLED 2026-09-19 IN FAVOUR OF THE DUAL-WIELDED MERGED MESH, which is now the proven path.
    //
    // The playtest that settled it: "dual-wielding with chrom works". Chrom's row is off, so his
    // jab falls back to the merged mesh -- and with `tools/dual-wield.py` putting a copy on
    // `HaveL` as well as `HaveR`, that fallback looks right in both facings. The article route
    // never got there: `sword` crashed the game outright, and these two render but wrong.
    //
    // So the article is the worse of two working-ish options for every weapon that HAS a merged
    // mesh, and the fallback is free -- `article_id` returns None for a disabled row and
    // `src/sword/mod.rs` shows the mesh pair instead, with no other change.
    //
    // `fusionsword` stays ON because it is the one article a playtest called fully working in both
    // directions, and `parachute` stays because it has no mesh to fall back to -- it never had one
    // (the canopy is four animated bones, which a rigid graft cannot show).
    // Corrin's up-air: "straight up in pikachu's body", reported twice.
    Candidate { host_name: "yato",        donor: "gamewatch", donor_article: "normal_weapon", weapon_kind: "WEAPON_KIND_GAMEWATCH_NORMAL_WEAPON", clean: true,  enabled: false,  authored: true,  model_from: "" },
    Candidate { host_name: "dragonhand",  donor: "kamui",     donor_article: "dragonhand",  weapon_kind: "WEAPON_KIND_KAMUI_DRAGONHAND",    clean: true,  enabled: false, authored: false, model_from: "" },
    // Samus's charge shot, added DISABLED 2026-09-19 purely to ask the donor-param audit
    // whether her projectile can be cloned at all. Her neutral-B fires
    // `FIGHTER_SAMUS_GENERATE_ARTICLE_CSHOT`, so without the article the move charges and
    // shoots nothing. `clean` is a claim the audit checks, not one made here.
    // ENABLED 2026-09-19, because the ported neutral-B needs it: Charge Shot without the article
    // is a charge with nothing to fire. The donor-param audit calls it clean in all 114 slots,
    // which is the fact that chose Samus over Piranha Plant -- see src/specials/chargeshot.
    //
    // IT HAS NO MODEL, AND THAT IS NOT A MISSING ASSET. `fighter/samus/model/cshot` is not a path
    // in the arc at all; only `motion/cshot/cNN/motion_list.bin` is. The visible ball is made
    // entirely of her effects, which ADR 0002 excludes, so the cloned projectile carries its
    // hitbox and its motion and is invisible. Stated here so a playtest reporting "the shot does
    // damage but you cannot see it" is a known cost rather than a new bug.
    // RE-ENABLED 2026-09-20 ONCE THE CRASH WAS PINNED ON THE BOW, NOT ON THIS.
    //
    // It was the first suspect and it was the wrong one, which is worth writing down: this is the
    // only article this project has cloned that has NO MODEL IN THE ARCHIVE AT ALL
    // (`fighter/samus/model/cshot` is not one of the 590,711 paths -- her shot is made of
    // effects), so "the novel configuration" looked like the answer. It ships
    // `motion/cshot/c00/motion_list.bin`, which is 1 of the archive's 1, and it is COMPLETE.
    // The bow was shipping 1 of 18. Novelty is not evidence.
    Candidate { host_name: "cshot",       donor: "samus",     donor_article: "cshot",       weapon_kind: "WEAPON_KIND_SAMUS_CSHOT",         clean: true,  enabled: true,  authored: false, model_from: "samus/supermissile" },
    // BYLETH'S BOW, AND HER N-AIR IS ENTIRELY MADE OF IT. `game_attackairn` generates the article,
    // `ArticleModule::change_motion(BOW, "attack_air_n")` drives it, and `remove_exist` takes it
    // away again -- all three are `FIGHTER_MASTER_GENERATE_ARTICLE_BOW` calls, which
    // tools/reach/src/bin/deforeign.rs strips because that id is 0 on Pikachu. So the move shipped
    // with its hitboxes and nothing to look at. Playtest: "byleth nair doesn't have visual".
    //
    // Both halves are already extracted locally (144 model paths, 68 motion), so unlike the charge
    // shot this needs no console session.
    // THIS ROW CRASHED THE GAME ON 2026-09-20, AND THE ARTICLE WAS NEVER THE PROBLEM -- THE FILES
    // WERE. A Data Abort at fault address 0 inside the engine's own fighter object construction,
    // reached through `smashline + 0x2a68c (fighter_initialize_object_data + 0x2c)`.
    //
    // The archive has EIGHTEEN files for `fighter/master/model/bow/c00`. `extracted/` held ONE.
    // tools/gen-article-assets.sh searches `work/arc-vis` first and falls back to `extracted/`,
    // and unlike cloud/fusionsword and gamewatch/parachute -- which were pulled off the console
    // complete, 14 of 14 and 16 of 16 -- the bow had no `arc-vis` copy. So it shipped a model
    // directory with a single file in it, and the engine dereferenced what was not there.
    //
    // All 18 are in `work/arc-vis` now, pulled with tools/arc-fetch.sh, and
    // tools/check-articlefiles.sh counts every installed article's shipped files against the
    // archive map so this class cannot ship again. check.sh's 3f could not see it: it compares
    // the copy against the source it was copied from, so a partial source is invisible to it.
    Candidate { host_name: "bow",         donor: "master",    donor_article: "bow",         weapon_kind: "WEAPON_KIND_MASTER_BOW",          clean: true,  enabled: true,  authored: false, model_from: "" },
    Candidate { host_name: "spikeball",   donor: "packun",    donor_article: "spikeball",   weapon_kind: "WEAPON_KIND_PACKUN_SPIKEBALL",    clean: false, enabled: false, authored: false, model_from: "" },
    Candidate { host_name: "rope",        donor: "gaogaen",   donor_article: "rope",        weapon_kind: "WEAPON_KIND_GAOGAEN_ROPE",        clean: false, enabled: true,  authored: false, model_from: "" },
    // G&W's FIREMEN AND TRAMPOLINE, which his special_hi main generates beside the parachute
    // (FIGHTER_GAMEWATCH_GENERATE_ARTICLE_RESCUE). Audits clean. LAST so no existing article id
    // moves. Its own agent scripts (`gamewatch_rescue`: the `up`/`down` visibility swap at frames
    // 1 and 8) run from lua2cpp_gamewatch, which smashline loads for the weapon's kind. Off until
    // its model is pulled into work/arc-vis/fighter/gamewatch/model/rescue.
    Candidate { host_name: "rescue",      donor: "gamewatch", donor_article: "rescue",      weapon_kind: "WEAPON_KIND_GAMEWATCH_RESCUE",    clean: true,  enabled: false, authored: false, model_from: "" },
];

/// Whether the clones are actually installed. A declared constant rather than commented-out calls,
/// because tools/check.sh greps for the call and a comment matched it.
///
/// KEEP THIS FALSE UNLESS SOMEBODY IS WATCHING THE CONSOLE. A crash on entry to a match is not
/// destructive, but it leaves the card in a state where the mod cannot be played.
// TURNED OFF 2026-09-19 AS A BISECT, not as a decision. Four consecutive boots failed -- three
// stalls and one crash -- and every theory tried so far has been eliminated by the next data
// point: SysDVR (not running for the second stall), the rotation probe (removing it changed
// nothing), the costume-slot expansion (the crash happened on the c00 build), and a canonical
// article-name collision between CHROM_SWORD and GANON_SWORD (ruled out because the Yato, hosted
// on GAMEWATCH_NORMAL_WEAPON but shipped as `yato`, was visibly seen twice).
//
// What the last crash DOES establish, from two byte-identical reports: it is deterministic, it is
// a null dereference inside the engine's article teardown (frame [04] sits in a function that
// calls `RemoveArticleByIndex`), and it happens during the DEMO/entry status -- the first status
// after a match loads. The plugin's own install completes cleanly every time; all six
// `clone_weapon` calls return before anything goes wrong.
//
// So the article system is where the crash lives, and this is the one switch that removes all of
// it at once. Off, the merged weapon meshes come back, which is the behaviour that shipped for
// many boots before articles existed -- so this is a PLAYABLE build as well as a measurement.
//
// It boots  -> articles are the cause, and the six rows bisect from the bottom up.
// It fails  -> articles are innocent, and the fault is somewhere this project has not looked.
// CLONING IS CLEARED OF THE AIR-DAMAGE CRASH. It was set false on 2026-10-17 to bisect it --
// clone_weapon is the only thing this port does that mutates SHARED engine state -- and the
// crash reproduced with "CLONE_ENABLED=false, nothing cloned" in the log. Restored.
pub const CLONE_ENABLED: bool = true;

/// Pikachu's own article count, so a cloned article's id can be computed instead of guessed.
///
/// Read out of the game's own `StaticFighterData` rather than counted by hand: the table
/// `get_static_fighter_data` (`main 0x64b730`) switches to is at `.data 0x5013ad8` for kind 0x8,
/// and its `static_article_info` lists six descriptors -- weapon kinds 0x4c, 0x4d, 0x52, 0x4f,
/// 0x50 and 0x51. So Pikachu's own articles are 0x0..0x5 and the first clone is 0x6.
pub const PIKACHU_VANILLA_ARTICLES: i32 = 6;

/// Per-instance neutralisation of a dirty row's offending vtable slots; see neutralize.rs.
mod neutralize;

/// Rows that will actually be cloned, in the order `install()` clones them.
///
/// A row the audit calls dirty may be installed only if neutralize.rs lists it, which gives each
/// of OUR instances a private vtable with the unguarded donor reads pointed at engine defaults.
/// tools/check-article-donor-params.sh fails unless that list covers every unguarded slot.
pub fn installed() -> impl Iterator<Item = (usize, &'static Candidate)> {
    CANDIDATES
        .iter()
        .filter(|c| c.enabled && (c.clean || neutralize::covers(c.host_name)))
        .enumerate()
}

/// The article id a host name ends up with, or None if it is not installed.
///
/// `smashline_clone_weapon` returns the index within the fighter's NEW_ARTICLES list, and the real
/// article id is the fighter's own article count plus that index -- so this is derivable from the
/// table and needs no runtime state, which is what makes it safe to call from a per-frame hook.
pub fn article_id(host_name: &str) -> Option<i32> {
    installed()
        .find(|(_, c)| c.host_name == host_name)
        .map(|(i, _)| PIKACHU_VANILLA_ARTICLES + i as i32)
}

/// The weapon kind for a row, resolved through the crate's `lua_const` table rather than written
/// as a number here. tools/check-luaconst.sh verifies those values against the game's own
/// registration code, so this is the one place the number can come from.
fn weapon_kind(c: &Candidate) -> Option<i32> {
    use smash::lib::lua_const::*;
    Some(match c.weapon_kind {
        "WEAPON_KIND_CHROM_SWORD" => *WEAPON_KIND_CHROM_SWORD,
        "WEAPON_KIND_ELIGHT_ESWORD" => *WEAPON_KIND_ELIGHT_ESWORD,
        "WEAPON_KIND_CLOUD_FUSIONSWORD" => *WEAPON_KIND_CLOUD_FUSIONSWORD,
        "WEAPON_KIND_GAMEWATCH_PARACHUTE" => *WEAPON_KIND_GAMEWATCH_PARACHUTE,
        "WEAPON_KIND_KAMUI_DRAGONHAND" => *WEAPON_KIND_KAMUI_DRAGONHAND,
        "WEAPON_KIND_PACKUN_SPIKEBALL" => *WEAPON_KIND_PACKUN_SPIKEBALL,
        "WEAPON_KIND_GAOGAEN_ROPE" => *WEAPON_KIND_GAOGAEN_ROPE,
        "WEAPON_KIND_GANON_SWORD" => *WEAPON_KIND_GANON_SWORD,
        "WEAPON_KIND_GAMEWATCH_NORMAL_WEAPON" => *WEAPON_KIND_GAMEWATCH_NORMAL_WEAPON,
        "WEAPON_KIND_SAMUS_CSHOT" => *WEAPON_KIND_SAMUS_CSHOT,
        "WEAPON_KIND_MASTER_BOW" => *WEAPON_KIND_MASTER_BOW,
        _ => return None,
    })
}

pub fn install() {
    // Every row is logged whether or not it is installed, because "what is this article's weapon
    // kind and what id did it get on this build" was a question nobody had written down, and the
    // log line lands BEFORE the risky call -- so the log itself says which article install()
    // reached. That is the difference between a bisectable crash and another anecdote.
    for c in CANDIDATES {
        let kind = weapon_kind(c);
        let id = article_id(c.host_name);
        crate::telemetry::sink::diag(format!(
            "[rat] articles: {:<11} {} {}/{} {}={} clean={} enabled={} id={}",
            c.host_name,
            // `authored` is the difference between "the game ships this article and we cloned it"
            // and "the game does not, so its model was built from the donor's body model and
            // hosted on this kind". Worth a word in the log: if an authored article misbehaves,
            // the geometry is ours and the suspicion belongs in tools/author-article.py, whereas a
            // cloned one is the arc's own bytes and the suspicion belongs in the engine.
            if c.authored { "AUTHORED on" } else { "cloned from" },
            c.donor,
            c.donor_article,
            c.weapon_kind,
            kind.map(|k| format!("{k:#x}")).unwrap_or_else(|| "UNRESOLVED".into()),
            c.clean,
            c.enabled,
            id.map(|i| format!("{i:#x}")).unwrap_or_else(|| "-".into()),
        ));
    }

    if !CLONE_ENABLED {
        crate::telemetry::sink::diag("[rat] articles: CLONE_ENABLED=false, nothing cloned".into());
        return;
    }

    // THE HOOK FIRST, so a covered-dirty row is cloned only if its protection is actually in
    // place. neutralize::install() verifies its hook site against the running game and stays
    // inactive on a mismatch; cloning the rope anyway would put an article in the game whose two
    // Incineroar-only slots nothing has repointed.
    neutralize::install();
    for (i, c) in installed() {
        if !c.clean && !neutralize::active() {
            crate::telemetry::sink::diag(format!(
                "[rat] articles: {} NOT cloned -- it needs the weapon hook, which did not verify on this game version",
                c.host_name));
            continue;
        }
        let Some(kind) = weapon_kind(c) else {
            crate::telemetry::sink::diag(format!(
                "[rat] articles: {} names {} which weapon_kind() cannot resolve -- SKIPPED",
                c.host_name, c.weapon_kind
            ));
            continue;
        };
        let idx = smashline::clone_weapon(c.donor, kind, "pikachu", c.host_name, true);
        crate::telemetry::sink::diag(format!(
            "[rat] articles: cloned {} -> clone_weapon returned {idx}, expected article id {:#x}",
            c.host_name,
            PIKACHU_VANILLA_ARTICLES + i as i32
        ));
    }
}

/// An installed article's own module accessor, through the engine's lookup and validated by
/// `crate::victim::object_boma` -- the same path `src/sword` uses, shared so the whip's rope can
/// face itself the way the native spawn does.
pub unsafe fn accessor(
    boma: *mut smash::app::BattleObjectModuleAccessor,
    id: i32,
) -> Option<*mut smash::app::BattleObjectModuleAccessor> {
    use smash::app::lua_bind::ArticleModule;
    let a = ArticleModule::get_article(boma, id);
    if a.is_null() {
        return None;
    }
    let object_id = smash::app::lua_bind::Article::get_battle_object_id(a) as u32;
    crate::victim::object_boma(object_id)
}

/// The rope can be spawned: its row is installed AND the weapon hook that makes it safe verified.
/// Every rope spawn goes through this.
pub fn rope_ready() -> bool {
    article_id("rope").is_some() && neutralize::active()
}

// ================================================================================================
// THE THREE ARTICLE OPS A PORTED ACMD IS ALLOWED TO MAKE
// ================================================================================================
//
// A verbatim ACMD carries its source's article id, and `tools/reach/src/bin/deforeign.rs` used to
// DELETE every statement that named one. That is right when the article is not cloned -- the id is
// a per-fighter enumeration and `FIGHTER_MASTER_GENERATE_ARTICLE_BOW` is 0, which on Pikachu is his
// electro ball -- and it is wrong when it IS cloned, because then there is a correct id to use and
// deleting the call throws the move's visual away.
//
// Byleth's n-air is the case that made the difference visible. The whole move is the bow:
//
//     ArticleModule::generate_article(BOW, false, -1)
//     ArticleModule::change_motion(BOW, hash40("attack_air_n"), false, -1.0)
//     ArticleModule::remove_exist(BOW, ARTICLE_OPE_TARGET_ALL)
//
// Three statements, all three stripped, so the port shipped her hitboxes and nothing to look at.
// Playtest: "byleth nair doesn't have visual".
//
// So deforeign now RETARGETS those calls at these wrappers instead of deleting them, passing the
// HOST NAME rather than an id. Keyed by name because the id is not a constant: it depends on which
// rows of CANDIDATES are enabled, and `article_id` derives it from the table at the moment of the
// call.
//
// EACH IS A NO-OP WHEN THE ROW IS OFF, which is the property that makes the retarget safe to apply
// unconditionally. Turning an article off then becomes exactly what it was before -- the move
// loses its visual and keeps its hitboxes -- instead of an article call with a wrong id, which is
// the failure that looks like a working move firing somebody else's projectile.
use smash::app::{lua_bind::ArticleModule, ArticleOperationTarget, BattleObjectModuleAccessor};
use smash::phx::Hash40;

pub unsafe fn generate_article(
    boma: *mut BattleObjectModuleAccessor,
    host: &str,
    flag: bool,
    idx: i32,
) {
    if let Some(id) = article_id(host) {
        ArticleModule::generate_article(boma, id, flag, idx);
    }
}

pub unsafe fn change_motion(
    boma: *mut BattleObjectModuleAccessor,
    host: &str,
    motion: Hash40,
    flag: bool,
    frame: f32,
) {
    if let Some(id) = article_id(host) {
        ArticleModule::change_motion(boma, id, motion, flag, frame);
    }
}

pub unsafe fn remove_exist(
    boma: *mut BattleObjectModuleAccessor,
    host: &str,
    target: ArticleOperationTarget,
) {
    if let Some(id) = article_id(host) {
        ArticleModule::remove_exist(boma, id, target);
    }
}

// THREE MORE, FOR THE UP-B'S PARACHUTE (2026-10-12). Game & Watch's SPECIAL_HI, _FALL and _CLOSE
// drive his parachute with `is_exist`, `remove` and `set_visibility_whole` as well as the three
// above -- tools/check-nativecalls.sh listed all of them as "not made" -- and the parachute row is
// installed, so they now have an id to act on. Same rule: a no-op when the row is off, and
// `is_exist` then says false, which is the truth about an article that was never installed.

pub unsafe fn is_exist(boma: *mut BattleObjectModuleAccessor, host: &str) -> bool {
    match article_id(host) {
        Some(id) => ArticleModule::is_exist(boma, id),
        None => false,
    }
}

pub unsafe fn remove(boma: *mut BattleObjectModuleAccessor, host: &str, target: ArticleOperationTarget) {
    if let Some(id) = article_id(host) {
        ArticleModule::remove(boma, id, target);
    }
}

pub unsafe fn set_visibility_whole(
    boma: *mut BattleObjectModuleAccessor,
    host: &str,
    visible: bool,
    target: ArticleOperationTarget,
) {
    if let Some(id) = article_id(host) {
        ArticleModule::set_visibility_whole(boma, id, visible, target);
    }
}
