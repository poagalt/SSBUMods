// Every source fighter's weapon, shown for its own move.
//
// TWO MECHANISMS, AND THE FIRST ONE IS THE GAME'S. A weapon that exists as a real article in the
// game is cloned onto Pikachu and GENERATED for the move; one that does not is still a mesh merged
// into his body model and toggled with `ModelModule::set_mesh_visibility`.
//
// WHY THE SPLIT EXISTS AT ALL, because the mesh route was the whole implementation until now.
// `docs/the-article-path.md` has the measurements; the outcome is that the mesh graft is
// GEOMETRICALLY CORRECT and still wrong facing left. The shipped `chrom_sword` vertices equal
// `source . local(Sword)` to 0.000001 over 444 vertices, which is the native attachment exactly,
// and nine other candidate causes were eliminated in the data. That build was deployed, verified
// byte-identical on the card, and played: unchanged. Facing is `rot_y_lr`, a rigid Y rotation,
// which cannot move one mesh relative to the rest of the model -- so whatever is left is not
// reachable from the model files, and the way out is to stop owning the transform.
//
// An article owns it instead. Its root bone IS the socket -- `chrom/model/sword` is two bones,
// `HaveR -> Sword` -- so the engine aligns the weapon model's own `HaveR` to the fighter's and
// takes the facing, the culling and the article's own `model.nuanmb` with it.
//
// WHICH WEAPONS CANNOT BE ARTICLES. Corrin's Yato is `Kamui_Weapon_VIS_O_OBJShape` inside
// `kamui/model/body` and Palutena's staff is `staff_group_VIS_O_OBJShape` inside
// `palutena/model/body`. Neither is an article in the game and neither has a `WEAPON_KIND_*` --
// palutena's three model directories are `body`, `gate` and `godwing`. They need an article model
// AUTHORED from the donor's body model, two bones, exactly Chrom's shape. Until that exists they
// stay on the mesh, which is why `Weapon` carries both names.
//
// THE FALLBACK IS DELIBERATE. If an article row in `src/articles/mod.rs` is disabled, `article_id`
// returns None and that weapon silently reverts to its merged mesh. So bisecting a bad boot by
// flipping one row off degrades to the behaviour that already shipped instead of showing nothing.
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

/// HOW AN ARTICLE GETS INTO THE HAND.
///
/// `ArticleModule::have(boma, article, joint, target, id, bool)`. It took seven eliminations to
/// find, and the reason it hid is that it is a call the OWNER makes -- so for Chrom it lives in
/// his fighter code, which this port does not run, and nothing was ever going to make it happen
/// by itself. What it is NOT, each checked: the descriptor (`extra` is `0x0` on all 35 articles
/// across 7 fighters), the article's own scripts (`lua2cpp_chrom` contains only `chrom`), a
/// `WeaponSpecializer` (no such symbol for Chrom), a kinetic type (202 `WEAPON_KINETIC_TYPE_*`
/// constants, none of them Chrom's), `param_<article>` (Chrom's `vl.prc` has 13 objects and none
/// of length 11), `on_init` (a one-shot placement from the owner's position, `lr`, team and
/// `scale`), or `set_sync_constraint_joint` (three callers game-wide, the real one passing
/// `hash40("top")`). And his sword's class overrides exactly 1 of its 114 vtable slots, that one
/// setting `lr`, a rotation and a visibility flag -- so there was never any follow in the
/// article's own code to find.
///
/// The probe is what made it findable: the parachute came back frozen at its spawn offset,
/// `all=(4.647,-2.699,-3.409)` identical on all seven sampled frames and differing only by which
/// way he faced when it appeared. A detached object placed once, which is exactly what
/// `generate_article` alone produces.
#[allow(dead_code)]
enum Attach {
    /// `ArticleModule::have` alone -- the engine's own mechanism, and what the game does.
    Have,
    /// PLACE the article at the owner's `socket` every frame -- position and facing, no `have`.
    ///
    /// TRIED ON THE PARACHUTE 2026-10-18 AND IT DOES NOT MOVE THE DRAWN MODEL. The probe read the
    /// article's root exactly on his origin every frame -- its skeleton followed -- while the
    /// playtest saw the canopy "stay in the same spot the whole time". `PostureModule::set_pos`
    /// moves what `joint_global_position` computes from, not what is rendered; only a LINK moves
    /// the render. Kept for the record, used by no row.
    Manual,
    /// The engine's general model constraint: `LinkModule::set_model_constraint_pos_ort` on the
    /// ARTICLE's own accessor, its `top` onto the owner's `socket`, position and orientation.
    ///
    /// Read off Piranha Plant's spikeball (`lua2cpp_packun/.../spikeball/status/Start/main.c`),
    /// which pins itself to its owner with exactly `(LINK_NO_CONSTRAINT, top, top,
    /// ORIENTATION | POSITION, true)` and no `link` call before it -- a generated article is
    /// already linked to its owner. Unlike `have`, it is not limited to item-holding joints, and
    /// `top` is every model's implicit root, so the article needs no bone of that name.
    ///
    /// TRIED ON THE PARACHUTE 2026-10-18 AND THE DRAWN MODEL STILL DID NOT FOLLOW ("still stays in
    /// the same place directly above pikachu"), while the probe again read its skeleton on his
    /// origin every frame. So the probe's joint reads do not see what is drawn, and whatever
    /// `have` sets up that this call does not -- most likely the link itself -- is what the
    /// renderer follows. Kept for the record, used by no row.
    Constrain,
    /// `have` onto `socket` -- the one call measured to move the DRAWN model -- then retarget that
    /// constraint's owner-side joint to `top` with `LinkModule::set_model_constraint_target_joint`,
    /// keeping the link `have` made. `have` itself refuses body joints, so this is the way to end
    /// up on his origin through a link the renderer follows. The link state is logged around both
    /// calls (`link_state`), so if the retarget does not take, the next build knows which link
    /// number and joints `have` used.
    HaveTop,
    /// `have` on spawn, then drive anyway. Only useful if `have` half-works.
    Both,
}

struct Weapon {
    /// The merged mesh object in Pikachu's body model, or "" when the weapon has no mesh.
    mesh: &'static str,
    /// The LEFT-HAND copy of that mesh, or "" when the weapon does not dual-wield.
    ///
    /// Pikachu holds a sword in BOTH hands for a sword move, and the reason is Part 9: `haver` is
    /// 3.958 units off his centreline in the lateral axis, so facing swings the hand through about
    /// eight units of camera depth on a body roughly two deep. One sword is in front of his
    /// silhouette facing one way and behind it facing the other, and no attachment mechanism fixes
    /// that because it is the rig, not the binding. Two swords mean one is always in front.
    ///
    /// `tools/dual-wield.py` appends these to the shipped model -- same vertices, same material,
    /// bound to `HaveL` instead of `HaveR` -- and `--verify` checks they stay vertex-identical to
    /// their right-hand copy. They are toggled together with `mesh` and never independently: a
    /// sword appearing in one hand and not the other would be worse than the problem.
    ///
    /// Empty for the parachute (no mesh at all) and for the staff (a staff is held in one hand;
    /// a second would read as a bug rather than as a flourish).
    mesh_l: &'static str,
    // WORTH INVESTIGATING (user request, 2026-09-19): hide whichever of the pair is on the FAR
    // side of the body and show only the near one, instead of always showing both.
    //
    // Dual-wielding works -- confirmed on every weapon -- but it is a workaround, not the native
    // look: the donor swings one sword and Pikachu now carries two. The visual is correct in both
    // facings because one of the pair is always in front; the cost is that the other is always
    // there too.
    //
    // The facing is already known every frame (`PostureModule::lr`), and `set_mesh_visibility`
    // already takes each mesh independently, so the change is small: show `mesh` when lr is one
    // sign and `mesh_l` when it is the other. What needs checking on hardware is whether the
    // swap reads cleanly at the moment he turns, and which sign maps to which hand -- `haver` is
    // 3.958 off the centreline in the lateral axis, so the near/far assignment follows directly
    // from lr's sign but is worth confirming rather than deriving.
    //
    // That would make the visuals entirely correct with one sword on screen, which is what the
    // game does.
    /// The cloned article's host name, or "" when the weapon is not an article in the game.
    article: &'static str,
    /// The bone ON PIKACHU this article is attached to, lowercase.
    ///
    /// `haver` for everything held, because that is the joint the whole port is measured against
    /// and the one the grafts bind to. It is deliberately per-weapon rather than a constant: the
    /// parachute's own root is `Have` and the dragon hand's is `Attach`, so if the engine's path
    /// turns out to key on the root bone name, this is where a different socket goes.
    socket: &'static str,
    /// A bone in the ARTICLE's own skeleton, for the probe below. Read out of each article's
    /// `model.nusktb` rather than guessed: `chrom/model/sword` is `HaveR -> Sword`,
    /// `cloud/model/fusionsword` is `HaveR -> fusionsword1..6`, `elight/model/esword` is
    /// `Root -> HaveR -> Sword1 -> Sword_lightA..F2`, and `gamewatch/model/parachute` is
    /// `Have -> Rot -> All -> Parachutea1/a2, b1/b2`. Lowercase, because bones are addressed by
    /// hash40 of the lowercased name.
    probe_bone: &'static str,
    /// The motions that show it -- its SOURCE fighter's moves, from `tools/moves.tsv`. A motion in
    /// two rows would show two weapons at once, which `tools/check-weapons.sh` refuses.
    motions: &'static [&'static str],
    /// TRUE WHEN THE ARTICLE OWNS MOTIONS OF ITS OWN NAMED AFTER THE MOVE, which selects a
    /// COMPLETELY DIFFERENT NATIVE MECHANISM from `have` -- and is the parachute's whole bug.
    ///
    /// The game attaches articles two ways, and which one it uses is decided by whether the
    /// article ships animations. Count the `.nuanmb` files in each article's motion directory:
    ///
    ///   chrom/motion/sword         `entryl`, `entryr`                  -- entry only
    ///   cloud/motion/fusionsword   NONE AT ALL, just `motion_list.bin`
    ///   elight/motion/esword       `specials1/s2`, `close/open`        -- none of its moves
    ///   gamewatch/motion/parachute `specialhi`, `specialairhi`, `specialhiopen`, `specialhiclose`
    ///
    /// The first three have no animation for the move that shows them, so they are RIGID and the
    /// engine keeps them in the hand -- which is what `ArticleModule::have` does, and why adding
    /// it took Cloud's back-air from "sword straight down through the model in both directions"
    /// to fully working.
    ///
    /// The parachute owns an animation for EVERY ONE of its four trigger motions, and G&W's own
    /// status code never calls `have` on it. What it does instead, in
    /// `lua2cpp_gamewatch/gamewatch/status/SpecialHi/main.c` and `SpecialHiFall/main.c`, is hand
    /// the article THE FIGHTER'S OWN MOTION HASH on the frame the fighter changes motion:
    ///
    ///     ArticleModule::change_motion(PARACHUTE, H, false, -1.0);
    ///     MotionModule::change_motion(H, 0.0, 1.0, false, 0.0, false, false);
    ///
    /// with `H` = `0xa28f17495` (`special_hi`, on the ground) or `0xed8a31e01` (`special_air_hi`,
    /// in the air) -- and `0xfedb5db59` (`special_hi_open`) once the drift status starts. Both
    /// hashes were confirmed by recomputing hash40 over the name, and they are exactly the four
    /// files in the motion directory. The article tracks the fighter because both are playing the
    /// same-named animation from the same frame; the canopy's position above the body is in the
    /// animation's own root track, authored in the owner's local space.
    ///
    /// So `have` was never the parachute's mechanism. It is why the canopy sat at the hand: `have`
    /// glued the root to `haver` and the hand is off-centre. This row gets the motion instead, and
    /// no `have` at all, which is what the game does.
    own_motion: bool,
    /// TRUE TO GIVE THE ARTICLE THE GAME'S OWN LIFETIME: generated once and kept for the whole
    /// match, with `set_visibility_whole` doing the showing and hiding.
    ///
    /// This is what "Chrom's sword ships only `entryl` and `entryr`" actually means. Those are
    /// ENTRY motions -- the article is created during his entry sequence and then simply exists,
    /// held in his hand, for every frame of the match. He never generates it per-move, which is
    /// why no `generate_article(CHROM_SWORD)` appears in any script or in his status code: there
    /// is nothing to generate at move time.
    ///
    /// This port has been doing the opposite -- `generate_article` on the frame the jab starts and
    /// `remove_exist` on the frame it ends -- and that article is the one that has answered
    /// `exist=false` on 32 consecutive samples across two boots while the three whose donors DO
    /// generate per-move came up on frame 1. That correlation is the reason to try the native
    /// lifetime, and it is cheap: `max_count` is 1, so a permanent instance costs one object.
    ///
    /// SET ON THE TWO ARTICLES WHOSE DONORS NEVER GENERATE THEM, AND ONLY THOSE. The decompiled
    /// status trees make this a controlled comparison rather than a hunch:
    ///
    ///   `CLOUD_FUSIONSWORD`   4 sites by `generate_article`/`remove_exist` -- per-move, WORKS
    ///   `GAMEWATCH_PARACHUTE` generated in `SpecialHi` and `SpecialHiFall` -- per-move, worked
    ///   `ELIGHT_ESWORD`       1 site, and it is an `is_exist` QUERY        -- persistent, broken
    ///   `CHROM_SWORD`         zero references in his ENTIRE status tree    -- persistent, broken
    ///
    /// Four for four, with a positive control: where the donor generates per-move, this port's
    /// per-move generate works; where the donor never does, it fails. So `sword` and `esword` get
    /// the native lifetime and `fusionsword` deliberately does NOT -- it currently works, and
    /// changing the lifetime of a weapon that works, unobserved, to fix one that does not is how
    /// this project has lost rounds before. `staff` and `yato` are hosted on kinds that ARE
    /// generated per-move (`GANON_SWORD`, 18 sites; `GAMEWATCH_NORMAL_WEAPON`, 26), so by the same
    /// correlation they should come up on the per-move path and are left on it.
    ///
    /// All three outcomes are informative and at most one is a regression: if generation still
    /// fails nothing changes at all, if it succeeds and visibility holds the move is fixed, and if
    /// it succeeds but visibility does not hold then Pikachu carries Falchion all match -- ugly,
    /// but it would prove generation works outside the move, which two boots could not.
    persist: bool,
    /// The ACMD generates and attaches this article; this row exists ONLY to guarantee it is
    /// taken away again.
    ///
    /// BYLETH'S BOW IS WHY. Her n-air IS the bow: the ported script generates it on frame 0 and
    /// calls `remove_exist` on frame 53. An ACMD stops executing the moment its status exits, and
    /// Pikachu leaves `attack_air_n` long before frame 53 -- so that removal line is never
    /// reached and the bow stays in his hand for the rest of the airtime. Landing hid it, because
    /// `src/landings` calls `remove_exist` too and that one does run.
    ///
    /// A row with an empty motion list could not fix it -- nothing would ever show the bow -- and
    /// a full row would change what already works, generating a frame early and `have`-ing it onto
    /// a joint the script never asked for. So this keeps generate-and-hold with the script that
    /// owns it and takes only the lifecycle guarantee.
    acmd_owned: bool,
    /// How this article follows him -- `have` onto `socket`, or placed at it every frame. Per row
    /// because the two kinds of article want different things: a held weapon is IN the hand, and
    /// the parachute hangs over the whole body. See `Attach`.
    attach: Attach,
}

const WEAPONS: &[Weapon] = &[
    // Chrom's Falchion -- his jab. `WEAPON_KIND_CHROM_SWORD`, and the simplest article in the
    // game: one entry in his whole descriptor table, `max_count` 1, and NO agent scripts --
    // `vendor/SSBU-Dumped-Scripts/smashline/lua2cpp_chrom` contains only `chrom`.
    Weapon { mesh: "chrom_sword", mesh_l: "chrom_sword_l", article: "sword", socket: "haver", probe_bone: "sword", // NO MOTIONS: Little Mac's jab replaced Chrom's, so Falchion has no move left to appear for.
    // The row STAYS rather than being deleted, and that is deliberate -- the mesh is still in the
    // model and its `numshexb` entry_flags say `draw_model: true`, so with no row nothing would
    // ever call `set_mesh_visibility(false)` on it and it would be permanently visible. An empty
    // motion list means `want` is always false, which hides it every frame.
    motions: &[], own_motion: false, persist: false, acmd_owned: false, attach: Attach::Have },
    // Cloud's Buster Sword -- his back-air. `WEAPON_KIND_CLOUD_FUSIONSWORD`, seven bones.
    Weapon { mesh: "cloud_sword", mesh_l: "cloud_sword_l", article: "fusionsword", socket: "haver", probe_bone: "fusionsword1", motions: &["attack_air_b"], own_motion: false, persist: false, acmd_owned: false, attach: Attach::Have },
    // Mythra's sword -- her dash attack and all three tilts. `WEAPON_KIND_ELIGHT_ESWORD`, eleven
    // bones, and the article carries `Sword_lightA..F2` which the graft had to flatten into one
    // bake because a grafted bone has no animation track.
    Weapon {
        mesh: "mythra_sword", mesh_l: "mythra_sword_l",
        article: "esword", socket: "haver",
        probe_bone: "sword1",
        // `attack_s3_*` LEFT WITH SHEIK, and `attack_air_lw` LEFT AGAIN FOR PYRA'S OWN BLADE.
        // Sheik's f-tilt is a kick and replaced Mythra's sword tilt, so the sword must not show
        // for it. Pyra's d-air briefly borrowed this mesh on the reasoning that Pyra and Mythra
        // are the same fighter pair -- they are not the same SWORD, hers is the flame form and
        // this is the light form, and the playtest said so. She has her own row now.
        motions: &["attack_dash"],
        own_motion: false,
        // TURNED OFF AFTER THE CRASH REPORT NAMED THIS PATH. 2026-09-18.
        //
        // The Atmosphere report for the up-B crash is a Data Abort, fault address 0, on
        // MainThread, and its stack is
        //
        //     rat_of_doom + 0x37f4
        //       cross2 + 0x3d41a8 / 0x3d4114   <- `bl GenerateArticleFromCopyAbilities`
        //       cross2 + 0x3a7000
        //       cross2 + 0xa804dc              <- blr x8
        //       cross2 + 0x33a17b8             <- ldr x0,[x8,#0x58]; ldr x8,[x0];
        //                                         ldr x8,[x8,#0x6e0]; blr x8
        //
        // and the final jump lands on 0x78d47430d0, which is in no loaded module. That is a
        // dispatch through a bad vtable inside ARTICLE GENERATION -- not `change_motion`, which
        // was the other new thing in that build and the one initially suspected.
        //
        // `persist` is what made that path dangerous. It moved generation from "during the move"
        // to "every frame of the entire match, including entry and respawn", and because Chrom's
        // sword never succeeds all THREE doors fire on every one of those frames. The same three
        // failing calls ran for many boots without crashing when they only ran during a jab, so
        // the difference is exposure, not the calls themselves.
        //
        // The reading that motivated it still stands (`ELIGHT_GENERATE_ARTICLE_ESWORD` appears
        // once in her whole status tree and it is an `is_exist`; Chrom's tree has no article
        // reference at all), so the native lifetime really is "created at entry, held forever".
        // Reproducing that is not a thing to do by retrying a failing generate every frame.
        persist: false, acmd_owned: false,
        attach: Attach::Have,
    },
    // G&W's parachute -- his up-B, all four motions of the sequence. THIS ONE HAS NO MESH AND
    // NEVER DID: the canopy opens on its own `model.nuanmb` (`Have -> Rot -> All ->
    // Parachutea1/a2, b1/b2`) and a grafted bone sits at bind pose, so the graft was never
    // attempted and the move has shipped with nothing to show. Its own agent drives the canopy
    // through `VisibilityModule::set_int64(para3/para4, on/off)`, which comes free with the clone.
    Weapon {
        mesh: "", mesh_l: "",
        // HISTORY: `have` at `haver` made it follow but left it at the off-centre HAND ("animated
        // correctly but statically offset"); `rot` stopped it following at all (2026-09-18, before
        // the 2026-10-12 ordering bug in `HELD` existed, so that refusal is real).
        // SOCKET `trans`, PLACED EVERY FRAME -- HIS ORIGIN, WHICH IS WHERE G&W'S PARACHUTE LIVES.
        //
        // MEASURED 2026-10-18, and every number below is from the game's own files or from the
        // probe on console #2, not inferred.
        //
        // `have` onto `have` WORKED: root-to-socket read (0.00,0.00,0.00) on every frame of every
        // up-B, both facings, visible throughout. It was the wrong place. Pikachu's `Have` is NOT
        // a hat point -- it is 3.61 units IN FRONT of his face (bind: Head z 0.36, Have z 3.97),
        // and his ported drift animation swings the head through z -6.5 .. +7.7 in held poses, so
        // a canopy on `Have` jumps from in front of him to behind him every ~28 frames.
        //
        // WHERE G&W PUTS IT. G&W has NO `Have` bone (45 bones; Trans, Rot, Hip, Neck, Head, HaveL,
        // HaveR), and his status code never calls `have`, so his parachute is not matched to any
        // joint by name. Its canopy carries its own height instead: `gamewatch/model/parachute`'s
        // meshes, through its `Rot` bone (56.2 degrees about X, +2.58 up), span y 7.3 .. 20.6
        // above the article's ROOT and roughly centred fore-aft, and their lowest points -- the
        // string ends -- sit at y 7.3, right where G&W's raised hands are (his `HaveR` is at 7.7).
        // So the root sits at his ORIGIN, the canopy is authored above it, and the hands meet the
        // strings because the meshes were modelled that way.
        //
        // `have` ONTO `have` IS THE ANSWER, AND THE PLAYTEST SAID SO BEFORE THIS ANALYSIS DID.
        // poagalt, 2026-10-18: "the first time you got the parachute to show up, it was correctly
        // moving back and forth with pikachu ... It's only since then that it's been static." That
        // first time was this attach. The swing that the measurements above read as a flaw -- the
        // canopy riding his head through the drift's stepped poses -- is the motion he wanted.
        // Three builds then moved it to his origin and every one left the DRAWN canopy static:
        // per-frame placement (`Attach::Manual`), the spikeball's constraint (`Attach::Constrain`),
        // and `have` retargeted to `top` (`Attach::HaveTop`, deployed, untested). Only `have`'s own
        // link is known to move what is drawn. Do not move this off `have` without a playtest.
        article: "parachute", socket: "have",
        probe_bone: "all",
        motions: &["special_hi", "special_air_hi", "special_hi_open", "special_hi_close"],
        // NOT `own_motion`: the up-B's status code (`src/specials/specialhi`, `specialhifall`,
        // `specialhiclose`) already hands the article its motion on each status change, exactly
        // as G&W's does, so this module only shows it and places it. Two callers of
        // `change_motion` would restart the canopy's animation.
        own_motion: false,
        persist: false, acmd_owned: false,
        attach: Attach::Have,
    },
    // Palutena's staff -- her neutral-air and her grab. The grab motions are hers too: the three
    // catch boxes in `moves.tsv` are Palutena's, and a staff grab with no staff reads as a bug.
    // NOT AN ARTICLE IN THE GAME -- see the header.
    // Palutena's staff -- her neutral-air and her grab. AUTHORED ARTICLE, hosted on
    // `WEAPON_KIND_GANON_SWORD`: her staff is `staff_group_VIS_O_OBJShape` inside
    // `palutena/model/body` and is not an article in the game, so tools/author-article.py builds
    // one -- two bones, `HaveR`(identity) -> `Stick`(her own local transform, a pure translation
    // of (0,-2.2,0) with no rotation at all), mesh copied across untouched. The merged mesh stays
    // as the fallback, which is what shows if the article does not come up.
    Weapon {
        mesh: "palutena_staff", mesh_l: "",
        article: "staff", socket: "haver",
        probe_bone: "stick",
        // NO MOTIONS, for the same reason as Chrom's sword above. Byleth's n-air replaced
        // Palutena's and Charizard's grab replaced hers, so every move the staff was shown for
        // now belongs to a fighter who does not carry one.
        motions: &[],
        own_motion: false,
        persist: false, acmd_owned: false,
        attach: Attach::Have,
    },
    // Corrin's Yato -- his up-air. NOT AN ARTICLE IN THE GAME: it is a rigid single-bound mesh in
    // his body model, which is why it was the only one of the five that grafted without adding a
    // bone at all.
    // Corrin's Yato -- his up-air. AUTHORED ARTICLE, hosted on `WEAPON_KIND_PICKEL_SWORD`. The
    // socket is `handr`, not `haver`: his weapon mesh is single-bound to HIS `HaveR`, so the
    // article's two bones are `HandR`(identity) -> `HaveR`(his own local transform), and aligning
    // that root to Pikachu's `handr` reproduces `v . local(HaveR) . world(HandR)` exactly.
    // SOCKET `haver`, CHANGED FROM `handr` AFTER THE PLAYTEST PUT IT ON HIS FEET. The article
    // generated and sat at the owner's origin, which is where an unattached article goes, so
    // `have` refused `handr` exactly as it refused `rot`. All fourteen of the game's own `have`
    // call sites pass `haver`; it is the only socket this project has ever seen work.
    // tools/author-article.py rebuilds the model to match -- one identity bone at `HaveR`.
    // Pyra's Aegis blade, flame form, for her d-air. `tools/graft-pyra-sword.py` appends it --
    // her mesh is SKINNED across `Sword1` and `Sword_flameA..E`, so it is flattened onto `HaveR`
    // by `inverse(world_esword(HaveR))`, which is merge-weapons.sh's own algebra for a skinned
    // source. Valid as one matrix because all 4611 of her weights are exactly 1.0 and no shipped
    // animation drives those bones.
    //
    // No article: `eflame`'s sword is a model directory, not a `WEAPON_KIND_*` this port clones,
    // and the merged mesh is the path that works anyway.
    Weapon { mesh: "pyra_sword", mesh_l: "pyra_sword_l", article: "", socket: "haver", probe_bone: "", motions: &["attack_air_lw"], own_motion: false, persist: false, acmd_owned: false, attach: Attach::Have },
    Weapon { mesh: "corrin_sword", mesh_l: "corrin_sword_l", article: "yato", socket: "haver", probe_bone: "haver", motions: &["attack_air_hi"], own_motion: false, persist: false, acmd_owned: false, attach: Attach::Have },
    // Byleth's bow -- her n-air, and the whole move is the bow. `WEAPON_KIND_MASTER_BOW`.
    //
    // NO MESH AND `acmd_owned`: it is a real article in the game, so nothing is merged into
    // Pikachu's body for it, and `src/aerials` already generates it and gives it her own
    // `attack_air_n` motion verbatim. All this row adds is the removal guarantee -- see
    // `Weapon::acmd_owned`. `landing_air_n` is in the list because `src/landings` re-aims the same
    // article, so the guard must not pull it out from under that.
    Weapon { mesh: "", mesh_l: "", article: "bow", socket: "haver", probe_bone: "",
        motions: &["attack_air_n", "landing_air_n"], own_motion: false, persist: false,
        acmd_owned: true, attach: Attach::Have },
];

/// THE PROBE THAT ANSWERS "DID THE ARTICLE TAKE", because the last three rounds could not.
///
/// Two boots have now come back "behaviour is the same", and the first of them was the same BUILD
/// -- the card's plugin was md5 `367475d6` while the local one was `00e6ce31`, and the card's
/// `model/` held `body` and a stale `spikeball` with none of the four article directories. That is
/// a deploy that did not happen, and nothing in the mod could have told anyone: a cloned article
/// that never installs makes `article_id` return None, `src/sword/mod.rs` silently falls back to
/// the merged mesh, and the result is byte-identical behaviour to the build before it.
///
/// So the fallback needs to be AUDIBLE. This logs, once per (frame, facing) for the first few
/// frames of each weapon's motion:
///
///   * whether an article id resolved at all -- if this says `id=none` the clone did not install,
///     and the question is the deploy or `CLONE_ENABLED`, not the engine;
///   * `is_exist` before and after `generate_article`, which separates "never asked" from "asked
///     and refused";
///   * the article's own bind bone, relative to the fighter's `haver`, with the lateral axis
///     multiplied by `lr` so a correctly-following article reads THE SAME both ways round. That
///     is the measurement the whole facing question turns on, and `next-boot.md` records
///     `joint_global_position` as unreliable for appended bones -- this reads the ARTICLE's own
///     skeleton through `ArticleModule::get_joint_pos`, which is a different path entirely.
///
/// Buffered through `sink::log`, not `sink::diag`: diag pays a file open and an SD flush per line
/// by design, and that is exactly the hitch that would make the mod differ from native.
const ARTICLE_PROBE: bool = true;
/// Sample every `PROBE_STRIDE`th frame of each motion, up to 32 samples per motion.
///
/// The first version took frames 0-7 and nothing else. For the parachute those are the first
/// frames of the RISE, when its own `game_specialhi` has both canopy meshes switched off -- it
/// measured the one stretch of the up-B where there is nothing to see, and never reached the
/// 120-frame drift where the canopy is actually out.
const PROBE_STRIDE: u32 = 4;
/// Motion episodes logged per weapon per player, so a long session does not fill the card.
const PROBE_EPISODES: u32 = 12;
/// Per player, per weapon: the motion being sampled, which sample slots it has logged, and how
/// many episodes have been logged so far. A new motion starts a new episode.
static mut PROBE_MOTION: [[u64; 8]; 8] = [[0; 8]; 8];
static mut PROBE_SEEN: [[u32; 8]; 8] = [[0; 8]; 8];
static mut PROBE_EPISODE: [[u32; 8]; 8] = [[0; 8]; 8];

/// Whether the CURRENT instance of each article has been through `hold`, per player.
///
/// THIS IS WHY NO PARACHUTE CAME OUT OF UP-B, measured 2026-10-18 on console #2. `hold` used to
/// run only inside the generate block -- "if I generated it, I hold it". Since 2026-10-12 the
/// up-B's own status code (`src/specials/specialhi`, transcribing G&W's `SpecialHi_main`)
/// generates the parachute first and HIDES it, and the status `main` runs before this per-frame
/// callback. So by the time this module looked, the article already existed, the generate block
/// was skipped, and with it the only `have` and the only `set_visibility_whole(true)` the
/// parachute ever got. The log said so in three ways: `exist=true` on frame 0 of every up-B, no
/// `ARTICLE parachute GENERATE` line in the whole session, and the lateral offset back at 4.647
/// -- the unattached value -- where `have` had once made it read 0.000.
///
/// So holding is keyed on the INSTANCE now, not on who made it: the first frame an article exists
/// while its move is running, it is held and shown, once. The flag drops the moment the article
/// stops existing, so the next instance is held again.
static mut HELD: [[bool; 8]; 8] = [[false; 8]; 8];
/// How many `hold` calls have been logged, so the line says the adoption ran without repeating.
static mut HOLD_LOGGED: u32 = 0;
/// How many link-state lines have been logged (three per adoption), for the same reason.
static mut LINK_LOGGED: u32 = 0;

/// The motion last handed to each `own_motion` article, so `change_motion` fires on the EDGE.
///
/// Zeroed when the article stops being live, so the next entry into the move re-applies it rather
/// than assuming the article kept a motion across a `remove_exist`.
static mut ARTICLE_MOTION: [u64; 8] = [0; 8];

/// How many generate attempts each weapon has logged, so a persistent article that keeps failing
/// does not write a line a frame for the whole match.
static mut GEN_LOGGED: [u32; 8] = [0; 8];

/// SHOWN FOR THE WHOLE MOTION, not only the hitbox-active frames.
///
/// Recorded as a deviation when the Chrom sword shipped and it still holds: popping the weapon in
/// and out mid-swing reads worse than a continuous swing, and the hitboxes are disjoint either way.
///
/// The article half is edge-triggered against `is_exist` rather than called every frame --
/// `generate_article` on an article that already exists is not a no-op worth relying on, and
/// `remove_exist` every frame would fight the article's own lifetime.
unsafe extern "C" fn pikachu_weapon_frame(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let motion = MotionModule::motion_kind(boma);
    let entry = (WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize).min(7);
    for (wi, w) in WEAPONS.iter().enumerate() {
        let want = w.motions.iter().any(|m| motion == smash::hash40(m));
        let id = if w.article.is_empty() {
            None
        } else {
            crate::articles::article_id(w.article)
        };

        if ARTICLE_PROBE && want && !w.article.is_empty() {
            probe(boma, wi, w, id);
        }

        match id {
            // The game's own mechanism.
            //
            // THE MESH IS HIDDEN ONLY WHEN THE ARTICLE IS ACTUALLY THERE, and the first version of
            // this hid it whenever an article id merely EXISTED IN THE TABLE. That cost a whole
            // playtest: `article_id` is derived from the table at compile time, so it returns
            // `Some` whether or not `clone_weapon` worked or `generate_article` succeeded -- and
            // the probe shows Chrom's sword answering `exist=false` on all 16 samples. So the
            // graft was suppressed and nothing replaced it, and the report came back "visually
            // gone from both left and right", which is strictly worse than the bug it replaced.
            //
            // Re-read after generating rather than reusing the pre-generate value: an article that
            // comes up on the same frame should not spend that frame with neither showing.
            Some(id) => {
                let exists = ArticleModule::is_exist(boma, id);
                // A PERSISTENT ARTICLE IS GENERATED WHETHER OR NOT THE MOVE IS RUNNING, because
                // that is when the game generates it: during entry, not on the move. See
                // `Weapon::persist`.
                // AN `acmd_owned` ARTICLE IS NEVER GENERATED HERE. The script that owns it already
                // does that, on the frame it chose and with the attachment it chose; all this block
                // would add is a duplicate a frame later. The removal below is NOT skipped, which is
                // the entire reason such a row exists.
                if (want || w.persist) && !exists && !w.acmd_owned {
                    // THE RETURN VALUE IS LOGGED because Chrom's sword has now answered
                    // `exist=false` on 32 consecutive samples across two boots while the other
                    // three come up on f1 or f2. `generate_article` returns a u64 nobody has ever
                    // looked at, and "the engine refused" and "the engine accepted and the article
                    // died immediately" are different problems with the same symptom.
                    let r = ArticleModule::generate_article(boma, id, false, -1);
                    let mut how = "generate_article";
                    // THREE WAYS IN, TRIED IN ORDER, because Chrom's sword refuses the first.
                    //
                    // `generate_article` has now answered `exist=false` on 32 consecutive samples
                    // for `WEAPON_KIND_CHROM_SWORD` while `esword`, `fusionsword` and `parachute`
                    // -- appended by the same loop, in the same order, ids 0x7/0x8/0x9 -- come up
                    // on f1 or f2. So the call is not wrong in general, it is wrong for that
                    // article, and the engine offers two other doors:
                    //
                    //   `generate_article_enable`         same shape, "enable" variant
                    //   `generate_article_have_item(joint)` generates AND holds, and its NAME is
                    //                                       the use case -- a held weapon
                    //
                    // Each only runs if the previous one left nothing behind, so on the three
                    // articles that already work this costs one `is_exist` and nothing else. The
                    // log says which door opened, which is the whole point of trying them in one
                    // boot instead of three.
                    if !ArticleModule::is_exist(boma, id) {
                        ArticleModule::generate_article_enable(boma, id, false, -1);
                        how = "generate_article_enable";
                    }
                    if !ArticleModule::is_exist(boma, id) {
                        ArticleModule::generate_article_have_item(
                            boma,
                            id,
                            -1,
                            Hash40::new(w.socket),
                        );
                        how = "generate_article_have_item";
                    }
                    // THROTTLED, because a persistent article that keeps failing retries every
                    // frame of the match and four lines say everything a thousand would.
                    if ARTICLE_PROBE && GEN_LOGGED[wi] < 4 {
                        GEN_LOGGED[wi] += 1;
                        crate::telemetry::sink::log(format!(
                            "ARTICLE {} GENERATE id={id:#x} first_returned={r:#x} via={how}                              exist_after={} generatable={}",
                            w.article,
                            ArticleModule::is_exist(boma, id),
                            ArticleModule::is_generatable(boma, id),
                        ));
                    }
                    // VISIBILITY AND WHO DRIVES THE MOTION ARE TWO QUESTIONS, and tying them to one
                    // flag is why no parachute came out of up-B.
                    //
                    // `own_motion` was turned off on the parachute after it crashed the game on
                    // 2026-09-18, which was right -- but it also silently took the article's only
                    // `set_visibility_whole(true)` with it, because that call sat in the `if`. So
                    // the parachute was cloned, generated, `have`-d onto his hand and left
                    // invisible, which reads in game as "up-B has no parachute" and reads in the
                    // log as a healthy article. Making it visible is unconditional now; whether the
                    // article or the fighter drives the motion is decided separately below.
                    ArticleModule::set_visibility_whole(
                        boma,
                        id,
                        true,
                        ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL),
                    );
                } else if !want && exists && !w.persist {
                    ArticleModule::remove_exist(
                        boma,
                        id,
                        ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL),
                    );
                }
                // HELD ONCE PER INSTANCE, WHOEVER GENERATED IT. See `HELD`. Before the `persist`
                // visibility below, so a persistent row still ends the frame showing `want`.
                //
                // NOT FOR `acmd_owned`: the script that generates those already decides where they
                // go, and a `have` onto a joint it never asked for would change what works.
                if !ArticleModule::is_exist(boma, id) {
                    HELD[entry][wi] = false;
                } else if !HELD[entry][wi]
                    && (want || w.persist)
                    && !w.acmd_owned
                    && !w.own_motion
                {
                    // G&W's own status code generated the parachute HIDDEN, so every path shows it.
                    match w.attach {
                        Attach::Constrain => constrain(boma, id, w),
                        Attach::HaveTop => have_then_top(boma, id, w),
                        Attach::Manual => {
                            ArticleModule::set_visibility_whole(
                                boma,
                                id,
                                true,
                                ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL),
                            );
                        }
                        Attach::Have | Attach::Both => hold(boma, id, w),
                    }
                    HELD[entry][wi] = true;
                    if ARTICLE_PROBE && HOLD_LOGGED < 16 {
                        HOLD_LOGGED += 1;
                        crate::telemetry::sink::log(format!(
                            "ARTICLE {} HOLD p{entry} socket={} generated_here={} status={:#x} motion={:#x} f={:.0}",
                            w.article,
                            w.socket,
                            !exists,
                            StatusModule::status_kind(boma),
                            motion,
                            MotionModule::frame(boma),
                        ));
                    }
                }
                // VISIBILITY IS WHAT CHANGES, for the same reason. `set_visibility_whole` is the
                // call G&W's own status code uses on the parachute, with `ARTICLE_OPE_TARGET_ALL`
                // (0) as the target, so it is the engine's idiom for exactly this and not a
                // workaround for not being able to remove the article.
                if w.persist && exists {
                    ArticleModule::set_visibility_whole(
                        boma,
                        id,
                        want,
                        ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL),
                    );
                }
                let live = want && ArticleModule::is_exist(boma, id);
                // THE ARTICLE PLAYS THE FIGHTER'S MOTION, edge-triggered on the motion changing.
                //
                // This is `lua2cpp_gamewatch`'s own sequence, and the edge matters as much as the
                // call: native changes the article's motion on the frame it changes the FIGHTER's,
                // once per status entry. Calling it every frame would restart the animation every
                // frame and leave the canopy frozen on frame 0 -- which is indistinguishable from
                // "not animated", the exact symptom the last two rounds produced by other means.
                //
                // `new_raw` because the hash is the one already read out of MotionModule, not a
                // name to re-hash. Pikachu's up-B motions are named `special_hi`/`special_air_hi`
                // like G&W's, so the fighter's own hash resolves in the article's motion list;
                // that is the same coincidence the native code relies on, not a mapping.
                // NOT ON THE FRAME IT WAS GENERATED, which is the one difference from the
                // version that crashed the game on up-B (2026-09-18).
                //
                // `exists` is read BEFORE the generate block, so gating on it defers the motion
                // by one frame: the article comes up on frame N playing whatever it defaults to,
                // and gets the fighter's motion on N+1. Native calls both on the same frame, but
                // native calls it from the status's own `main`, which runs at a defined point in
                // the article's lifetime; this is a per-frame line callback on the FIGHTER and
                // has no such guarantee, so calling into a module that was constructed moments
                // earlier in the same frame is the one thing here that could plausibly touch
                // something not yet initialised.
                //
                // This is a CANDIDATE, not a diagnosis -- the crash report has not been read. It
                // is written now so that re-enabling `own_motion` re-enables the safer shape, and
                // the cost if it is wrong is one frame of default motion.
                if live && exists && w.own_motion && ARTICLE_MOTION[wi] != motion {
                    ARTICLE_MOTION[wi] = motion;
                    ArticleModule::change_motion(boma, id, Hash40::new_raw(motion), false, -1.0);
                } else if live && !w.own_motion && matches!(w.attach, Attach::Manual | Attach::Both) {
                    drive(boma, id, w);
                }
                if !live {
                    ARTICLE_MOTION[wi] = 0;
                }
                if !w.mesh.is_empty() {
                    ModelModule::set_mesh_visibility(boma, Hash40::new(w.mesh), want && !live);
                    if !w.mesh_l.is_empty() {
                        ModelModule::set_mesh_visibility(
                            boma, Hash40::new(w.mesh_l), want && !live);
                    }
                }
            }
            // No article installed for this weapon -- either it has none in the game, or its row
            // is disabled. Fall back to the merged mesh.
            None => {
                if !w.mesh.is_empty() {
                    ModelModule::set_mesh_visibility(boma, Hash40::new(w.mesh), want);
                    if !w.mesh_l.is_empty() {
                        ModelModule::set_mesh_visibility(boma, Hash40::new(w.mesh_l), want);
                    }
                }
            }
        }
    }
}

/// An article's own module accessor, through the engine's own lookup.
///
/// This used to read `(*article).module_accessor` off smash-rs's `Article` struct, which is
/// `{ vtable, module_accessor, generate_id }` by smash-rs's reading of the layout and nobody's
/// measurement. `lua_bind::Article::get_battle_object_id` is the game's, and
/// `crate::victim::object_boma` makes the accessor confirm its own id, so a wrong layout or a
/// stale handle comes back null instead of as an arbitrary pointer. It matters
/// because everything that reaches the ARTICLE's modules rather than the fighter's goes through
/// this, and they are different objects.
unsafe fn article_boma(
    boma: *mut BattleObjectModuleAccessor,
    id: i32,
) -> *mut BattleObjectModuleAccessor {
    let a = ArticleModule::get_article(boma, id);
    if a.is_null() {
        return std::ptr::null_mut();
    }
    let object_id = smash::app::lua_bind::Article::get_battle_object_id(a) as u32;
    crate::victim::object_boma(object_id).unwrap_or(std::ptr::null_mut())
}

/// THE ENGINE'S OWN ATTACHMENT, found at last: `ArticleModule::have`.
///
/// Snake's cardboard box is a held article, and every one of his four appeal scripts attaches it
/// with exactly this, verbatim from `lua2cpp_snake/snake/AppealHiL.txt`:
///
///     ArticleModule::have(agent.module_accessor, *FIGHTER_SNAKE_GENERATE_ARTICLE_CBOX,
///                         Hash40::new("haver"), ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL),
///                         *BATTLE_OBJECT_ID_INVALID as u32, false);
///
/// A joint name, and `haver` is the joint. That is the whole mechanism, and it is why none of the
/// seven places it was hunted for had it: it is not the descriptor, not the article's scripts, not
/// a `WeaponSpecializer`, not a kinetic type, not `param_<article>`, not `on_init`, and not
/// `set_sync_constraint_joint` -- it is an ArticleModule call the OWNER makes, so for Chrom it
/// lives in his fighter code, which this port does not run. Nothing was going to make it happen
/// by itself.
///
/// It also subsumes the rotation problem `drive` below cannot solve. The engine is doing the whole
/// transform here, so the blade should sit in the hand at the angle the article's own skeleton
/// specifies -- no per-weapon Euler correction to derive.
unsafe fn hold(boma: *mut BattleObjectModuleAccessor, id: i32, w: &Weapon) {
    ArticleModule::have(
        boma,
        id,
        Hash40::new(w.socket),
        ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL),
        *BATTLE_OBJECT_ID_INVALID as u32,
        false,
    );
    // AND FORCED VISIBLE, because a held article can start hidden and Chrom's does.
    //
    // Mapping PostureModule's and VisibilityModule's vtable slots back to their lua_bind names
    // (28 and 19 of them respectively, recovered from the four-instruction thunks) makes his
    // sword's ONE vtable override readable, and it says:
    //
    //     PostureModule::set_scale(...)       slot 0x120
    //     PostureModule::set_rot(...)         slot 0x0e0
    //     VisibilityModule::set_whole(0)      slot 0x0c0   <- hides the whole model
    //
    // So the article the engine builds for `WEAPON_KIND_CHROM_SWORD` hides itself, and something
    // in Chrom's own fighter code is what shows it again. Nothing here would.
    //
    // Diddy and DK drive exactly this call for their held articles -- barreljets, gun, dkbarrel --
    // so it is the game's own idiom and not a workaround, and it is harmless on an article that
    // was already visible.
    ArticleModule::set_visibility_whole(
        boma,
        id,
        true,
        ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL),
    );
}

/// Drive the article's posture from the owner's socket joint, every frame.
///
/// WHAT THIS GETS RIGHT: the article tracks the hand in both facings, because both the position
/// and the rotation are read from the socket's own world transform and the whole point of
/// `joint_global_*` is that it already has the facing in it. That alone answers the parachute,
/// which the probe caught frozen at its spawn offset -- `all=(4.647,-2.699,-3.409)` identical on
/// all seven sampled frames, differing only by which way he faced when it appeared.
///
/// WHAT IT DOES NOT GET RIGHT, AND THIS IS THE KNOWN GAP. Natively the blade sits at
/// `v . local(bind_bone) . world(socket)`, where `local(bind_bone)` is the article's own root-to-
/// blade transform -- for Chrom that is `HaveR -> Sword` with `Sword`'s matrix being `HaveR`'s
/// transposed, so `world_article(Sword)` is the identity and the whole orientation comes from
/// that one constant. Setting the article's rotation to the socket's world rotation omits it, so
/// the blade tracks the hand but sits at a fixed wrong angle inside it. That is exactly the shape
/// of "the sword is straight down through the model in both directions": a constant rotation
/// error, not a facing error.
///
/// The correction is a per-weapon constant Euler derived from each article's own `model.nusktb`,
/// which is offline data work and is the next step. It is left out rather than guessed because
/// composing Euler angles by eye is how this project has lost rounds before -- and because a
/// weapon that tracks the hand at the wrong angle is legible enough to measure the angle from.
/// `Attach::HaveTop`. See the variant.
unsafe fn have_then_top(boma: *mut BattleObjectModuleAccessor, id: i32, w: &Weapon) {
    let abm = article_boma(boma, id);
    link_state(abm, w, "before-have");
    hold(boma, id, w);
    link_state(abm, w, "after-have");
    if !abm.is_null() {
        LinkModule::set_model_constraint_target_joint(abm, Hash40::new("top"));
    }
    link_state(abm, w, "after-retarget-top");
}

/// One line of the article's link state: whether it has a model constraint, which constraint/link
/// number, the article-side and owner-side joint hashes, and which link numbers point at a parent.
/// hash40 for reference: have 0x4d27eea40, haver 0x51a07c0e7, top 0x31ed91fca, trans 0x570211ebd.
unsafe fn link_state(abm: *mut BattleObjectModuleAccessor, w: &Weapon, when: &str) {
    if !ARTICLE_PROBE || abm.is_null() || LINK_LOGGED >= 12 {
        return;
    }
    LINK_LOGGED += 1;
    let mut links = String::new();
    for ln in 0..8 {
        if LinkModule::is_link(abm, ln) {
            links.push_str(&format!(" {ln}->{:#x}", LinkModule::get_parent_object_id(abm, ln)));
        }
    }
    crate::telemetry::sink::log(format!(
        "ARTICLE {} LINK {when}: model_constraint={} no={:#x} joint={:#x} target={:#x} links=[{}]",
        w.article,
        LinkModule::is_model_constraint(abm),
        LinkModule::get_model_constraint_no(abm),
        LinkModule::get_model_constraint_joint(abm),
        LinkModule::get_model_constraint_target_joint(abm),
        links.trim(),
    ));
}

/// `Attach::Constrain`: the article's `top` onto the owner's `socket`, once per instance, then
/// shown. The engine keeps it there from then on, the same way it keeps a `have`-d sword in hand.
unsafe fn constrain(boma: *mut BattleObjectModuleAccessor, id: i32, w: &Weapon) {
    let abm = article_boma(boma, id);
    if !abm.is_null() {
        LinkModule::set_model_constraint_pos_ort(
            abm,
            *WEAPON_LINK_NO_CONSTRAINT,
            Hash40::new("top"),
            Hash40::new(w.socket),
            (*CONSTRAINT_FLAG_ORIENTATION | *CONSTRAINT_FLAG_POSITION) as u32,
            true,
        );
    }
    ArticleModule::set_visibility_whole(
        boma,
        id,
        true,
        ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL),
    );
}

unsafe fn drive(boma: *mut BattleObjectModuleAccessor, id: i32, w: &Weapon) {
    let abm = article_boma(boma, id);
    if abm.is_null() {
        return;
    }
    // POSITION AND FACING, AND NOTHING ELSE. This used to copy the socket joint's rotation too,
    // through `joint_global_rotation`, whose output convention nobody measured -- and for the one
    // row that uses this, the parachute, the right orientation is simply his facing: its canopy
    // is authored upright in his frame, which is how G&W's sits over G&W.
    let mut p = Vector3f { x: 0.0, y: 0.0, z: 0.0 };
    ModelModule::joint_global_position(boma, Hash40::new(w.socket), &mut p, true);
    PostureModule::set_pos(abm, &p);
    PostureModule::set_lr(abm, PostureModule::lr(boma));
    PostureModule::update_rot_y_lr(abm);
}

/// ONE-SHOT ARTICLE CENSUS, logged the first time any ported weapon's motion runs.
///
/// Chrom's sword has now answered `exist=false` on 32 consecutive samples across two boots while
/// `esword`, `fusionsword` and `parachute` -- ids `0x7`, `0x8`, `0x9`, appended by the same loop in
/// the same order -- all come up on f1 or f2. The id mapping is not the problem: if it were off by
/// one, up-B would have generated a sword instead of a parachute, and a parachute is what appeared.
///
/// So the question is what the engine thinks about `0x6` specifically, and it will answer if asked.
/// `is_generatable`, `get_num` and `get_active_num` are all bound, and Pikachu's own six articles
/// are the controls: whatever a healthy article looks like, `0x0`..`0x5` look like it.
static mut CENSUS_DONE: bool = false;

unsafe fn census(boma: *mut BattleObjectModuleAccessor) {
    if CENSUS_DONE {
        return;
    }
    CENSUS_DONE = true;
    for id in 0..=9i32 {
        let tag = match id {
            0..=5 => "pikachu-own",
            _ => "cloned",
        };
        crate::telemetry::sink::log(format!(
            "ARTICLE-CENSUS id={id:#x} {tag:<11} generatable={} exist={} num={} active={}",
            ArticleModule::is_generatable(boma, id),
            ArticleModule::is_exist(boma, id),
            ArticleModule::get_num(boma, id),
            ArticleModule::get_active_num(boma, id),
        ));
    }
}

/// Sample one weapon on one frame. Called only while its motion is running and only for weapons
/// that name an article, so an unported move costs nothing.
unsafe fn probe(
    boma: *mut BattleObjectModuleAccessor,
    wi: usize,
    w: &Weapon,
    id: Option<i32>,
) {
    let entry = {
        let e = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
        if e >= 8 { 0 } else { e }
    };
    if wi >= 8 {
        return;
    }
    census(boma);
    let frame = MotionModule::frame(boma);
    if frame < 0.0 {
        return;
    }
    let motion = MotionModule::motion_kind(boma);
    if PROBE_MOTION[entry][wi] != motion {
        PROBE_MOTION[entry][wi] = motion;
        PROBE_SEEN[entry][wi] = 0;
        PROBE_EPISODE[entry][wi] += 1;
    }
    if PROBE_EPISODE[entry][wi] > PROBE_EPISODES {
        return;
    }
    let f = frame as u32;
    let slot = f / PROBE_STRIDE;
    if slot >= 32 {
        return;
    }
    let bit = 1u32 << slot;
    if PROBE_SEEN[entry][wi] & bit != 0 {
        return;
    }
    PROBE_SEEN[entry][wi] |= bit;
    let lr = PostureModule::lr(boma);

    let Some(id) = id else {
        crate::telemetry::sink::log(format!(
            "ARTICLE {} f{f} lr{:+.0} id=none -- NOT INSTALLED, falling back to mesh {:?}",
            w.article, lr, w.mesh
        ));
        return;
    };

    let exist = ArticleModule::is_exist(boma, id);
    // THE OWNER'S SOCKET AND HEAD, in world space, through the same call every fighter-side
    // number in this project was measured with.
    let mut sock = Vector3f { x: 0.0, y: 0.0, z: 0.0 };
    // `top` is the model's implicit root, not a skeleton bone; `trans` sits at the same point.
    let sock_bone = if w.socket == "top" { "trans" } else { w.socket };
    ModelModule::joint_global_position(boma, Hash40::new(sock_bone), &mut sock, true);
    let mut hd = Vector3f { x: 0.0, y: 0.0, z: 0.0 };
    ModelModule::joint_global_position(boma, Hash40::new("head"), &mut hd, true);

    let where_ = if !exist || w.probe_bone.is_empty() {
        "-".to_string()
    } else {
        // THE ARTICLE'S OWN BONES THROUGH THE ARTICLE'S OWN ModelModule, which is the same call
        // as the owner's side above -- so both ends of every difference come from one API.
        //
        // `ArticleModule::get_joint_pos` is still printed (`gjp`) but no longer trusted. On the
        // 2026-10-18 capture it put the WORKING fusion sword's first bone 62 units below the hand
        // on every frame, which no bone on an 11-unit fighter can be; whatever space it reports
        // in, it is not the one `joint_global_position` does.
        //
        // The article's accessor comes from the engine's own lookup --
        // `lua_bind::Article::get_battle_object_id` then `sv_battle_object::module_accessor` --
        // and not from reading `(*article).module_accessor`, whose offset is smash-rs's guess at
        // a struct layout. A wrong guess there is an arbitrary pointer.
        let gjp = ArticleModule::get_joint_pos(
            boma,
            id,
            Hash40::new(w.probe_bone),
            ArticleOperationTarget(*ARTICLE_OPE_TARGET_FIRST),
        );
        let abm = article_boma(boma, id);
        if abm.is_null() {
            format!("abm=null gjp=({:.2},{:.2},{:.2})", gjp.x, gjp.y, gjp.z)
        } else {
            // ROOT vs SOCKET is the attachment test: `have` pins the article's root bone to the
            // socket, so a held article reads ~(0,0,0) here on every frame and a loose one
            // drifts as he moves. The parachute's root is `Have`; the swords' is `HaveR`.
            let root = if w.article == "parachute" { "have" } else { "haver" };
            let mut ar = Vector3f { x: 0.0, y: 0.0, z: 0.0 };
            ModelModule::joint_global_position(abm, Hash40::new(root), &mut ar, true);
            // PROBE BONE vs HEAD is the question the playtest asks -- "above Pikachu" -- with the
            // lateral axis times `lr`, so a correctly-following bone reads the same both ways.
            let mut ap = Vector3f { x: 0.0, y: 0.0, z: 0.0 };
            ModelModule::joint_global_position(abm, Hash40::new(w.probe_bone), &mut ap, true);
            let pos = *PostureModule::pos(abm);
            let mut tr = Vector3f { x: 0.0, y: 0.0, z: 0.0 };
            ModelModule::joint_global_position(boma, Hash40::new("trans"), &mut tr, true);
            format!(
                "root-sock=({:.2},{:.2},{:.2}) root-trans=({:.2},{:.2},{:.2}) {}-head=({:.2},{:.2},{:.2}) \
                 apos-head=({:.2},{:.2},{:.2}) vis={} head=({:.1},{:.1}) gjp=({:.2},{:.2},{:.2})",
                (ar.x - sock.x) * lr,
                ar.y - sock.y,
                ar.z - sock.z,
                (ar.x - tr.x) * lr,
                ar.y - tr.y,
                ar.z - tr.z,
                w.probe_bone,
                (ap.x - hd.x) * lr,
                ap.y - hd.y,
                ap.z - hd.z,
                (pos.x - hd.x) * lr,
                pos.y - hd.y,
                pos.z - hd.z,
                u8::from(VisibilityModule::is_visible(abm)),
                hd.x,
                hd.y,
                gjp.x,
                gjp.y,
                gjp.z
            )
        }
    };
    crate::telemetry::sink::log(format!(
        "ARTICLE {} f{f} lr{:+.0} st={:#x} id={id:#x} exist={exist} held={} {where_}",
        w.article,
        lr,
        StatusModule::status_kind(boma),
        u8::from(HELD[entry][wi]),
    ));
}

pub fn install() {
    crate::charaslot::agent("pikachu")
        .on_line(Main, pikachu_weapon_frame)
        .install();
}
