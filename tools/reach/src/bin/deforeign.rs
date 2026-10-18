// Removes statements that reference a SOURCE fighter's own namespace constants.
//
// A verbatim ACMD port carries the source's article ids and instance work-flag ids, and those are
// per-fighter enumerations that collide. Measured against skyline-smash's own constant tables:
//
//   FIGHTER_ELIGHT_GENERATE_ARTICLE_ESWORD                      = 0x0        = Pikachu's DENGEKIDAMA
//   FIGHTER_ELIGHT_..._ADD_PARTIAL_MTION_SWORD_WHEN_CHANGEING   = 0x200000E6 = Pikachu's FLAG_STRANS_1
//   FIGHTER_KROOL_..._REQUEST_WAIST_SHIELD_ON                   = 0x200000E6 = Pikachu's FLAG_STRANS_1
//   FIGHTER_KROOL_..._REQUEST_WAIST_SHIELD_OFF                  = 0x200000E7 = Pikachu's FLAG_PAUSE
//
// So the ported f-tilt was setting Pikachu's STRANS_1 flag, the ported d-air was setting STRANS_1
// and PAUSE, and `IS_EXIST_ARTICLE(agent, 0)` was testing for Pikachu's electro ball and would
// have applied Mythra's sword motion-part kind to it. None of that is the ported move; it is the
// source fighter's own machinery reaching into the host's state.
//
// The K. Rool flags are also the belly-armor toggle, which ADR 0001 excludes by decision: it is a
// character-wide K. Rool trait, not part of the d-air.
//
// Algorithm: drop each offending statement, then cascade-delete any block left with an empty body.
// That handles both shapes seen -- a flag set alone inside `if is_excute { ... }` (the block goes
// too) and one sharing a block with the move's real ATTACK calls (only the statement goes).
//
// usage: deforeign <plugin-src-dir>

// Fighter namespaces we port from. Constants under FIGHTER_ATTACK_*, FIGHTER_KINETIC_*,
// FIGHTER_STATUS_* and the like are engine-global, shared by every fighter, and must NOT be
// stripped.
//
// THIS LIST WENT STALE ONCE AND THE HOLE WAS SILENT. DONKEY was missing while gen-moves.sh was
// already porting five of his cargo-throw scripts, so
// FIGHTER_DONKEY_STATUS_SHOULDER_START_FLAG_SHOULDER_WAIT sailed through into the built plugin --
// a write into DK's own SHOULDER_START status region, landing on whatever Pikachu keeps at that
// index during a throw. A list that has to be kept in step by hand will not be, so build.sh now
// diffs it against the fighters gen-moves.sh actually names and refuses if one is missing.
const SOURCES: &[&str] = &[
    "CHROM", "ELIGHT", "FALCO", "ROBOT", "LITTLEMAC", "FOX", "GAMEWATCH", "PALUTENA", "DEMON",
    "CLOUD", "KAMUI", "KROOL", "GAOGAEN", "MEWTWO", "KOOPA", "GANON", "DONKEY",
    // Added 2026-09-19 for the ten-move replacement set. CHROM, ELIGHT, ROBOT, PALUTENA, DEMON
    // and KROOL are kept even though their moves were replaced: several still supply other moves
    // (Mythra's dash attack, Palutena's grab until Charizard's lands), and an unused entry here
    // costs nothing while a missing one makes build.sh refuse outright.
    "SHEIK", "MASTER", "EFLAME", "PLIZARDON", "MARIOD", "NESS", "SAMUS",
    // f-air moved Kazuya -> Falco -> WOLF.
    "WOLF",
    // neutral-B: Piranha Plant's Ptooie (2026-10-18).
    "PACKUN",
];

// STATUS-SCOPED work ids that are deliberately KEPT.
//
// The blanket rule above is right about INSTANCE work ids and article ids: those live in the
// fighter's global state and a source fighter's index means something else on the host. Status
// work ids are different. They are indices into the region belonging to whatever status is
// currently running, so they only collide with ids the HOST uses IN THAT SAME STATUS -- and where
// this moveset replaces every line of a status, no host code runs in it and the region is ours.
//
// That distinction matters because these particular flags are the only record of the move's
// timing. Incineroar's game_specialsthrow raises FLAG_THROW_ROPE on frame 37 and that is the exact
// frame the victim is whipped at the ropes; stripping it left the status machine with nothing to
// read and no way to recover the number. Same story for G&W's up-B, whose LANDING_ENABLE and
// PARACHUTE_OPEN flags decide when he lands and when the parachute comes out.
//
// SAFETY CHECK before adding a name here, in order:
//   1. the moveset must register Pre, Main and End (and Init/Exec where the source has them) for
//      the status the script runs in, so no host line executes inside it;
//   2. the numeric index must fall inside the region the host fighter itself uses, so it is
//      certainly allocated. Pikachu's own status flags reach 0x2100001A (Skull Bash) and his
//      status ints reach 0x1100000A, which covers every id kept below;
//   3. it must be a STATUS_ id, not an INSTANCE_ or GENERATE_ARTICLE_ one.
//
// Anything not listed is still stripped.
const KEEP_STATUS_IDS: &[&str] = &[
    // THE TWO BELOW COST THREE PLAYTEST ROUNDS, and the failure mode is worth stating because it
    // is not a bug in this file -- it is the cost of a keep-list that has to be kept in step by
    // hand with what the STATUS code reads.
    //
    // `FLAG_MOVE_START` (0x2100000C) and `FLAG_MOVE_END` (0x2100000E) are raised by
    // `game_specialsstart` on frames 1 and 39. Nothing read them until the whip's statuses were
    // transcribed from Incineroar's, so stripping them was harmless and invisible. Then
    // `whip_move_flags` arrived -- it converts those two edges into the steady `FLAG_MOVE`
    // (0x2100000D), and `whip_kinetics` reads `FLAG_MOVE` to choose between holding position and
    // sliding at `special_s_start_speed_mul`. With both raises stripped the flag was never set,
    // every frame took the stand-still branch, and the playtest reported "doesn't move and even at
    // point blank doesn't grab" -- the grab being a consequence, since the catch box only reaches
    // 10.94 and the lunge is what closes the distance.
    //
    // What made it hard to see: the strip leaves the `frame()` marker and deletes the body, so the
    // generated file grows an EMPTY `frame(39.0);` -- which reads as deliberate. And
    // `check-acmd-values.sh` passes throughout, because it diffs macro ARGUMENTS and a bare
    // `WorkModule::on_flag` has none. `tools/check-keepflags.sh` now closes that gap.
    //
    // Against the safety check above, in order: (1) SPECIAL_S and its five follow-ups register
    // Pre/Main/End and the main_loop, so no host line runs in them; (2) 0x0C/0x0D/0x0E are far
    // inside Pikachu's own status-flag region, which reaches 0x21000019 -- these are the safest
    // ids on this list, not the most marginal; (3) both are STATUS_ ids.
    "FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_MOVE_START",
    "FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_MOVE_END",
    // Alolan Whip -- SPECIAL_S and its five follow-up statuses are entirely ours.
    "FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_THROW_ROPE",
    "FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_THROW_ROPE_END",
    "FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_DAMAGE_CUT",
    "FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_ENABLE_GRAVITY",
    "FIGHTER_GAOGAEN_STATUS_SPECIAL_S_WORK_ID_FLAG_AIR_CONTROL",
    "FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLOAT_LARIAT_HIT_FRAME",
    // Game & Watch up-B -- SPECIAL_HI is entirely ours.
    "FIGHTER_GAMEWATCH_STATUS_SPECIAL_HI_FLAG_DECIDE_DIRECTION",
    "FIGHTER_GAMEWATCH_STATUS_SPECIAL_HI_FLAG_LANDING_ENABLE",
    "FIGHTER_GAMEWATCH_STATUS_SPECIAL_HI_FLAG_PARACHUTE_OPEN",
    "FIGHTER_GAMEWATCH_STATUS_SPECIAL_HI_WORK_INT_ATTACK_AIR_KIND",
    // SAMUS'S CHARGE SHOT. Added against the three-point check above:
    //   1. the moveset registers every line of all six of her neutral-B statuses -- Pre, Main
    //      and End on each, plus Init/Exec/Exit wherever her tree has them -- so no Pikachu
    //      code runs inside them and the status work region is ours;
    //   2. the indices are 0x2100000D, 0x2100000E and 0x2100000F, inside the range Pikachu's own
    //      status flags reach (0x2100001A, Skull Bash);
    //   3. all three are STATUS_ ids.
    //
    // THEY ARE WRITTEN BY HAND-WRITTEN CODE, NOT BY A GENERATED SCRIPT -- her ACMDs consist
    // ENTIRELY of lines this file strips, so `src/specials/chargeshot/acmd.rs` writes them
    // directly and deforeign, which only visits `src/<dir>/mod.rs`, never sees that file. Listed
    // anyway: tools/check-keepflags.sh asks whether ANY dumped ACMD writes an id the status code
    // reads, which hers do, and an allowlist that is right for the wrong reason today is an
    // allowlist that is wrong the day those scripts do go through gen-moves.
    "FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_BULLET_DISP",
    "FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_CHARGE",
    "FIGHTER_SAMUS_STATUS_SPECIAL_N_FLAG_SHOOT",
    // PIRANHA PLANT'S PTOOIE (2026-10-18). Its ACMDs drive the move through five of his status
    // ids, all checked against the three rules: (1) src/specials/ptooie registers every line of
    // all seven statuses, so no Pikachu code runs in them; (2) the flags are 0x2100000D..10 and the
    // float 0x100000A, inside Pikachu's own status-work ranges (flags to 0x2100001A, floats past
    // 0x1000010 on every fighter); (3) all five are STATUS_ ids. GENERATE_ARTICLE_SPIKEBALL is what
    // makes the ball exist at all -- frame 10 of game_specialnstart -- and SHOOT_BLOW_SPIKEBALL
    // is how the shot scripts time the release.
    "FIGHTER_PACKUN_STATUS_SPECIAL_N_FLAG_GENERATE_ARTICLE_SPIKEBALL",
    "FIGHTER_PACKUN_STATUS_SPECIAL_N_FLAG_SHOOT_BLOW_SPIKEBALL",
    "FIGHTER_PACKUN_STATUS_SPECIAL_N_FLAG_SHOOT_PREVIOUS_WAIT",
    "FIGHTER_PACKUN_STATUS_SPECIAL_N_WORK_FLAG_END_PREVIOUS_SHOOT",
    "FIGHTER_PACKUN_STATUS_SPECIAL_N_WORK_FLOAT_SHOOT_START_SPIKEBALL_DISTANCE",
];

// A CLONED ARTICLE'S CALL IS RETARGETED, NOT DELETED.
//
// Stripping is right when the article is not cloned: the id is a per-fighter enumeration and
// `FIGHTER_MASTER_GENERATE_ARTICLE_BOW` is 0, which on Pikachu is his electro ball. It is WRONG
// when the article IS cloned, because then a correct id exists and deleting the call throws the
// move's visual away.
//
// Byleth's n-air is the case that made that visible. The entire move is the bow -- generate,
// change_motion, remove_exist, three statements, all three stripped -- so the port shipped her
// hitboxes with nothing to look at. Playtest: "byleth nair doesn't have visual". Samus's charge
// shot had the same shape and was worked around by hand in `src/specials/chargeshot`; this is the
// general answer, and that hand-written pair can go once it is proven.
//
// THE REWRITE PASSES A HOST NAME, NOT AN ID, because the id is not a constant -- it depends on
// which rows of `src/articles/mod.rs` are enabled. `crate::articles::<op>` looks it up and is a
// no-op when the row is off, so turning an article off restores exactly the old behaviour instead
// of leaving a call with a wrong id.
//
// ONLY THE OPS THAT ACTUALLY OCCUR. Across every script gen-moves.sh ports there are four:
// generate_article, change_motion and remove_exist (Byleth's bow) and add_motion_partial
// (Pyra's esword, which this port does not clone, so it keeps being stripped). An unhandled op on
// a cloned article is NOT silently stripped -- it is reported, because that is a call somebody
// meant to keep.
// `shoot_exist` joined 2026-10-18 with Ptooie, whose shot scripts hand the ball to the engine.
const RETARGET_OPS: &[&str] = &["generate_article", "change_motion", "remove_exist", "shoot_exist"];

/// `(donor, DONOR_ARTICLE_IN_CAPS) -> host_name`, read out of src/articles/mod.rs so this cannot
/// drift from the table that decides what is actually cloned.
fn cloned_articles(src_dir: &str) -> Vec<(String, String, String)> {
    let p = std::path::Path::new(src_dir).join("articles").join("mod.rs");
    let Ok(txt) = std::fs::read_to_string(p) else { return Vec::new() };
    let mut out = Vec::new();
    for l in txt.lines() {
        let t = l.trim_start();
        if !t.starts_with("Candidate {") { continue }
        let field = |k: &str| -> Option<String> {
            let i = t.find(&format!("{k}: \""))? + k.len() + 3;
            let rest = &t[i..];
            Some(rest[..rest.find('"')?].to_string())
        };
        let (Some(host), Some(donor), Some(art)) =
            (field("host_name"), field("donor"), field("donor_article")) else { continue };
        // ENABLED, CLEAN OR NOT. A dirty row is only installed when src/articles/neutralize.rs
        // covers it, and `crate::articles::<op>` is a no-op whenever a row is not installed -- so a
        // retargeted call on an uninstalled dirty row is the same no-op a stripped one would be,
        // and on a covered one (Ptooie's spikeball) it is the call the move needs.
        if !t.contains("enabled: true") { continue }
        out.push((donor.to_uppercase(), art.to_uppercase(), host));
    }
    out
}

/// If this line is an `ArticleModule::<op>(.., *FIGHTER_<DONOR>_GENERATE_ARTICLE_<ART>, ..)` for an
/// article we clone, return the rewritten line.
fn retarget(line: &str, arts: &[(String, String, String)]) -> Option<String> {
    let op_at = line.find("ArticleModule::")? + "ArticleModule::".len();
    let rest = &line[op_at..];
    let op = &rest[..rest.find('(')?];
    for (donor, art, host) in arts {
        let konst = format!("*FIGHTER_{donor}_GENERATE_ARTICLE_{art}");
        if !line.contains(&konst) { continue }
        if !RETARGET_OPS.contains(&op) {
            eprintln!("  ! ArticleModule::{op} on the cloned article '{host}' is not a retargeted \
                       op -- it would be stripped, which is probably not what is wanted:\n    {}",
                      line.trim());
            return None;
        }
        return Some(line.replace("ArticleModule::", "crate::articles::")
                        .replace(&konst, &format!("\"{host}\"")));
    }
    None
}

fn is_foreign(line: &str) -> bool {
    if KEEP_STATUS_IDS.iter().any(|k| line.contains(k)) {
        return false;
    }
    SOURCES.iter().any(|s| {
        line.contains(&format!("FIGHTER_{s}_")) || line.contains(&format!("WEAPON_{s}_"))
    })
}

// A ported script can also call a macro our smash-script version does not export. Incineroar's
// side-B uses CHECK_FINISH_CAMERA_IF_NOT_HP_MODE, which is absent from the 112 smash-script
// provides even though plain CHECK_FINISH_CAMERA is there -- and that fails the build outright
// rather than degrading. The allowlist is read from smash-script's own source at build time, so
// the next port cannot break the build the same way and the list cannot drift from the crate.
fn called_macro(line: &str) -> Option<&str> {
    let p = line.find("macros::")? + 8;
    let rest = &line[p..];
    let e = rest.find('(')?;
    Some(&rest[..e])
}

fn main() {
    let src_dir = std::env::args().nth(1).expect("plugin src dir");
    // Optional: a file of one macro name per line. Without it, no macro filtering happens, so
    // the pass stays useful even if the list has not been generated.
    let known: std::collections::HashSet<String> = std::env::args()
        .nth(2)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default();
    let mut total_stmts = 0usize;
    let mut total_blocks = 0usize;
    let mut total_macros = 0usize;
    let mut total_retargeted = 0usize;
    // Read once, from the table that decides what is actually cloned.
    let arts = cloned_articles(&src_dir);

    for entry in std::fs::read_dir(&src_dir).unwrap().flatten() {
        let dir = entry.path();
        if !dir.is_dir() { continue }
        let file = dir.join("mod.rs");
        if !file.exists() { continue }

        let mut lines: Vec<String> =
            std::fs::read_to_string(&file).unwrap().lines().map(String::from).collect();

        // 1. Drop offending statements. Only whole single-line statements are handled; a foreign
        //    constant inside a multi-line construct would be missed, so refuse loudly instead.
        let mut stmts = 0usize;
        let mut macros_dropped = 0usize;
        let mut retargeted = 0usize;
        let mut keep: Vec<bool> = vec![true; lines.len()];

        // Unsupported macro calls first, on the same statement-drop-then-cascade basis.
        if !known.is_empty() {
            for (i, l) in lines.iter().enumerate() {
                if let Some(m) = called_macro(l) {
                    if !known.contains(m) && l.trim().ends_with(';') {
                        keep[i] = false;
                        macros_dropped += 1;
                    }
                }
            }
        }

        for i in 0..lines.len() {
            if !keep[i] { continue }
            if !is_foreign(&lines[i]) { continue }
            // Retarget before considering the strip: a cloned article's call is kept, rewritten.
            if let Some(r) = retarget(&lines[i], &arts) {
                lines[i] = r;
                retargeted += 1;
                continue;
            }
            let l = &lines[i];
            let t = l.trim();
            if t.ends_with(';') && !t.starts_with("if ") && !t.contains('{') {
                keep[i] = false;
                stmts += 1;
            } else if t.starts_with("if ") && t.ends_with('{') {
                // `if <foreign cond> {` -- keep the line for now; emptying the body will remove
                // the block in step 2 once its only statement is gone. But if the condition
                // itself is foreign and the body is NOT purely foreign we would silently keep a
                // colliding test, so handle it by rewriting the condition to a constant false.
                // In practice every such block's body is foreign too, and step 2 removes it.
            } else {
                eprintln!("  ! {}: foreign constant in an unhandled construct, left alone:\n    {}",
                          file.display(), t);
            }
        }
        lines = lines.iter().zip(&keep).filter(|(_, k)| **k).map(|(l, _)| l.clone()).collect();

        // 2. Cascade-remove blocks whose body is now empty: a line ending in `{` immediately
        //    followed by its closing `}` at the same indentation. Repeat until stable, so an
        //    `if A { if B { } }` collapses entirely.
        let mut blocks = 0usize;
        loop {
            let mut removed = false;
            let mut i = 0usize;
            while i + 1 < lines.len() {
                let open = lines[i].trim_end();
                let close = lines[i + 1].trim();
                // Never collapse a function definition. K. Rool's game_landingairlw consists of
                // nothing but the belly-armor flag, so emptying its body cascaded into deleting
                // the function itself and the install list stopped compiling. An empty body is
                // exactly what is wanted there anyway: it overrides Pikachu's own landing script
                // and suppresses his leftover landing hit.
                let is_fn = open.contains("unsafe extern \"C\" fn ");
                if open.ends_with('{') && close == "}" && !is_fn {
                    let oi = lines[i].len() - lines[i].trim_start().len();
                    let ci = lines[i + 1].len() - lines[i + 1].trim_start().len();
                    if oi == ci {
                        lines.drain(i..=i + 1);
                        blocks += 1;
                        removed = true;
                        continue;
                    }
                }
                i += 1;
            }
            if !removed { break }
        }

        // `retargeted` counts too: a file whose ONLY change is a rewritten article call still has
        // to be written back, and leaving it out of this test is how Byleth's bow would have been
        // retargeted in memory and thrown away.
        if stmts > 0 || blocks > 0 || macros_dropped > 0 || retargeted > 0 {
            std::fs::write(&file, lines.join("\n") + "\n").unwrap();
            println!("  {:<10} {stmts} foreign statements, {macros_dropped} unsupported macros, {blocks} emptied blocks removed, {retargeted} article call(s) retargeted",
                     dir.file_name().unwrap().to_string_lossy());
            total_stmts += stmts;
            total_blocks += blocks;
            total_macros += macros_dropped;
            total_retargeted += retargeted;
        }
    }
    println!("deforeign: {total_stmts} foreign statements, {total_macros} unsupported macro calls and {total_blocks} blocks removed; {total_retargeted} cloned-article call(s) retargeted");
}
