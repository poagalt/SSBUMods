// Compares every NON-GEOMETRIC argument of a ported ACMD's macro calls against the source's.
//
// WHAT NOTHING WAS CHECKING. tools/check-hitboxes.sh measures where a hitbox is and how big it is.
// It says nothing about the other thirty arguments of `macros::ATTACK` -- damage, angle, KBG, FKB,
// BKB, hitlag, SDI, set-off kind, collision attribute, sound level -- and nothing at all about
// `ATTACK_ABS`, which carries the damage and knockback of every DK cargo throw and has no geometry
// for the trace to see. "Exact frame data, damage, and range from each move's source character" is
// the project's own statement of intent, and only `range` had a gate.
//
// The values are copied verbatim by tools/gen-moves.sh, so this should always pass -- which is the
// point. Two passes in this pipeline rewrite the generated scripts afterwards:
// tools/reach/src/bin/deforeign.rs deletes whole statements and blocks containing another
// fighter's constants, tools/reach/src/bin/sysfx.rs collapses effect blocks, and
// tools/reach/src/bin/topscale.rs REBUILDS EVERY ATTACK ARGUMENT LIST from parsed spans. Any of
// the three could alter or drop a value, and none of their output was compared against the input.
//
// COMPLEMENTARY, NOT OVERLAPPING. Geometry arguments -- size, x/y/z and the stretch endpoints --
// are deliberately skipped: topscale is supposed to change those, and check-hitboxes.sh owns them,
// measuring the result in world units against the source's own skeleton. Splitting it this way
// means neither check has to know whether SCALE_BONE_RADII is on.
//
// usage: acmdparity <source.txt> <source_fn> <ours.rs> <our_fn>
use std::collections::HashSet;

/// (call, argument indices that are geometry and therefore not compared here).
///
///   ATTACK(agent, id, group, bone, dmg, angle, kbg, fkb, bkb, size, x, y, z, x2, y2, z2, ...)
///   CATCH (agent, id,        bone, size, x, y, z, x2, y2, z2, status, mask)
///   ATTACK_ABS carries damage and knockback and NO position or radius, so nothing is skipped --
///   every argument of it must match, which is exactly what makes the cargo throws checkable.
const CALLS: &[(&str, &[usize])] = &[
    ("macros::ATTACK(", &[9, 10, 11, 12, 13, 14, 15]),
    ("macros::CATCH(", &[3, 4, 5, 6, 7, 8, 9]),
    ("macros::ATTACK_ABS(", &[]),
    // Timing and state, all of which are exact values with no geometry in them. FT_MOTION_RATE is
    // game frames per motion frame, so a wrong one is a move that runs at the wrong speed --
    // reproduced by hbtrace's walk for hitbox frames, and unchecked everywhere else in the script.
    ("macros::FT_MOTION_RATE(", &[]),
    ("macros::FT_MOTION_RATE_RANGE(", &[]),
    ("macros::WHOLE_HIT(", &[]),
    // EFFECTS. sysfx.rs keeps sys_ ones and drops fighter-owned ones (ADR 0002), and it currently
    // drops zero -- so every effect call that survives should be byte-identical to the source's.
    // If a future port makes it drop something, the call-count check names both sequences and the
    // right answer is a declared exception, not a looser comparison.
    ("macros::EFFECT(", &[]),
    ("macros::EFFECT_FLIP(", &[]),
    ("macros::EFFECT_FOLLOW(", &[]),
    ("macros::EFFECT_FLIP_ALPHA(", &[]),
    ("macros::EFFECT_BRANCH_SITUATION(", &[]),
    ("macros::LANDING_EFFECT(", &[]),
    ("macros::LANDING_EFFECT_FLIP(", &[]),
    ("macros::FOOT_EFFECT(", &[]),
    ("macros::CHECK_FINISH_CAMERA(", &[]),
    // The shoulder half of the Alolan Whip uses the stamina-mode-aware variant, which
    // smash-script does not export and src/acmd_ext therefore supplies. It was being deleted by
    // deforeign and nothing noticed, because a macro absent from this table is invisible to the
    // parity check -- so listing it here is half the fix.
    ("macros::CHECK_FINISH_CAMERA_IF_NOT_HP_MODE(", &[]),
    ("macros::REVERSE_LR(", &[]),
    ("macros::ATK_HIT_ABS(", &[]),
    ("macros::SET_SPEED_EX(", &[]),
    // THE BANG MACROS, AND ONE OF THEM IS LITTLE MAC'S SUPER ARMOR.
    //
    // `damage!(agent, MA_MSC_DAMAGE_DAMAGE_NO_REACTION, DAMAGE_NO_REACTION_MODE_ALWAYS, 0)` on
    // script frame 8 of his f-smash, back to _MODE_NORMAL on 14, IS the armor -- the one named
    // exception to ADR 0001's no-character-traits rule and the only non-hitbox property this
    // project committed to shipping. It was easy to believe it was missing, because it is not
    // spelled "armor" anywhere and grepping for that word finds nothing.
    //
    // Twenty `damage!`, five `grab!` and four `notify_event_msc_cmd!` calls across the generated
    // scripts, and none of them was checked by anything.
    ("damage!(", &[]),
    ("grab!(", &[]),
    ("search!(", &[]),
    ("slope!(", &[]),
    ("notify_event_msc_cmd!(", &[]),
];

/// Splits a macro call's arguments on top-level commas, so Hash40::new("x") and Some(1.0) survive.
fn args_of(line: &str, call: &str) -> Option<Vec<String>> {
    let p = line.find(call)? + call.len();
    let rest = &line[p..];
    let mut depth = 0i32;
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, c) in rest.char_indices() {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => {
                out.push(rest[start..i].trim().to_string());
                return Some(out);
            }
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(rest[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    None
}

/// Every tracked macro call inside one function, in source order.
fn calls_in(text: &str, want: &str) -> Vec<(&'static str, Vec<String>)> {
    let mut infn = false;
    let mut out = Vec::new();
    for line in text.lines() {
        if line.contains("unsafe extern \"C\" fn ") {
            infn = line.contains(&format!("fn {want}("));
        }
        if !infn {
            continue;
        }
        for (call, _) in CALLS {
            if let Some(v) = args_of(line, call) {
                out.push((*call, v));
            }
        }
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 4 {
        eprintln!("usage: acmdparity <source.txt> <source_fn> <ours.rs> <our_fn>");
        std::process::exit(2);
    }
    let mut src = std::fs::read_to_string(&a[0]).unwrap_or_default();
    // `--effect-work-map CONST=name`, repeatable: an EFFECT_*_WORK call reads its effect from a
    // work slot only the DONOR's code fills, so tools/port-effects.py writes the named effect that
    // slot holds instead (ADR 0007). The source is rewritten the same way before comparing, and
    // only for the constants listed.
    for (i, x) in a.iter().enumerate() {
        if x != "--effect-work-map" {
            continue;
        }
        let Some((c, n)) = a.get(i + 1).and_then(|p| p.split_once('=')) else { continue };
        for m in ["EFFECT_FOLLOW", "EFFECT_OFF_KIND"] {
            src = src.replace(
                &format!("macros::{m}_WORK(agent, *{c},"),
                &format!("macros::{m}(agent, Hash40::new(\"{n}\"),"),
            );
        }
    }
    let ours = std::fs::read_to_string(&a[2]).unwrap_or_default();
    let (sfn, ofn) = (&a[1], &a[3]);

    // A MISSING FUNCTION AND AN EMPTY ONE ARE DIFFERENT THINGS, and only one is a failure.
    //
    // K. Rool's `game_landingairlw` genuinely has no tracked calls -- it only sets his waist-shield
    // flag, which ADR 0001 excludes as a character-wide trait -- so our empty version is correct.
    // A misspelled function name produces exactly the same "no tracked calls", and that is a bug.
    // Asking whether the name is in the file at all separates them.
    for (label, text, name) in [("source", &src, sfn), ("ours", &ours, ofn)] {
        if !text.contains(&format!("fn {name}(")) {
            println!("  no fn {name} in the {label} file -- check the acmd/fn columns in moves.tsv");
            std::process::exit(3);
        }
    }

    let s = calls_in(&src, sfn);
    let o = calls_in(&ours, ofn);

    let mut bad = 0usize;
    let mut checked = 0usize;

    // SUBSET MODE, for the effect scripts, where dropping is the correct behaviour.
    //
    // ADR 0002 says the mod must work without CSK, so tools/reach/src/bin/sysfx.rs keeps only
    // sys_ effects and drops every fighter-owned one. Measured: Mythra's dash attack loses five
    // of its eight calls. An exact comparison therefore fails by design and says nothing -- but
    // there is still a real question underneath it, and it had no answer: does sysfx ever ALTER a
    // call instead of dropping it?
    //
    // So every call we KEEP must appear in the source, identically, in order. Drops are free;
    // changes and inventions are not.
    let rebased = a.iter().any(|x| x == "--rebased");
    // `--bone-alias from=to`, repeatable. See the note at the comparison below.
    let aliases: Vec<(String, String)> = a
        .iter()
        .enumerate()
        .filter(|(_, x)| *x == "--bone-alias")
        .filter_map(|(i, _)| a.get(i + 1))
        .filter_map(|p| p.split_once('=').map(|(f, t)| (f.to_string(), t.to_string())))
        .collect();
    // `--effect-sub from=to`, repeatable -- one per row of tools/effect-subs.tsv. See the
    // comparison below.
    let effect_subs: Vec<(String, String)> = a
        .iter()
        .enumerate()
        .filter(|(_, x)| *x == "--effect-sub")
        .filter_map(|(i, _)| a.get(i + 1))
        .filter_map(|p| p.split_once('=').map(|(f, t)| (f.to_string(), t.to_string())))
        .collect();
    // `--effect-joint-map from=to[:rotz90]`, repeatable: the joint translations tools/port-effects.py
    // applies (ADR 0007). `feeler=head` is a rename; `sword1=haver:rotz90` also moves the offset
    // from Sword1's frame into HaveR's -- Pyra's and Mythra's Sword1 sits on HaveR with no
    // translation and a 90-degree turn about Z, so (x,y,z) -> (-y,x,z) and rz -> rz+90. The
    // comparison applies the SAME rule to the source call and then still checks every argument,
    // so a port that translated wrongly, or anything else it changed, still fails.
    let joint_maps: Vec<(String, String, bool)> = a
        .iter()
        .enumerate()
        .filter(|(_, x)| *x == "--effect-joint-map")
        .filter_map(|(i, _)| a.get(i + 1))
        .filter_map(|p| {
            let (f, rest) = p.split_once('=')?;
            let (t, rot) = match rest.split_once(':') {
                Some((t, r)) => (t, r == "rotz90"),
                None => (rest, false),
            };
            Some((f.to_string(), t.to_string(), rot))
        })
        .collect();
    let num = |s: &String| s.trim().parse::<f32>().ok();
    let joint = |s: &String| {
        s.trim()
            .trim_start_matches("Hash40::new(\"")
            .trim_end_matches("\")")
            .to_string()
    };
    // The source call rewritten by the joint maps, or None if no map applies.
    let translate = |sc: &str, sa: &Vec<String>| -> Option<Vec<String>> {
        let mut out = sa.clone();
        let mut hit = false;
        // (joint index, first position index, rotation-z index) for each pose the call carries.
        let poses: Vec<(usize, usize, Option<usize>)> = if sc.contains("AFTER_IMAGE4_ON") && sa.len() >= 22 {
            vec![(4, 5, None), (8, 9, None), (14, 15, Some(20))]
        } else if sc.contains("EFFECT") && !sc.contains("LAST_EFFECT_SET") && sa.len() >= 9 {
            vec![(2, 3, Some(8))]
        } else {
            vec![]
        };
        for (j, p, rz) in poses {
            let Some((_, to, rot)) = joint_maps.iter().find(|(f, _, _)| joint(&sa[j]) == *f) else { continue };
            out[j] = format!("Hash40::new(\"{to}\")");
            hit = true;
            if *rot {
                let (x, y) = (num(&sa[p])?, num(&sa[p + 1])?);
                out[p] = format!("{}", -y);
                out[p + 1] = format!("{}", x);
                if let Some(r) = rz {
                    out[r] = format!("{}", num(&sa[r])? + 90.0);
                }
            }
        }
        if hit { Some(out) } else { None }
    };
    // Arguments equal as text, or as numbers to 1e-4 -- the port prints its translated numbers
    // with Python's formatting, the dump with the decompiler's.
    let same_arg = |x: &String, y: &String| -> bool {
        x.trim() == y.trim()
            || matches!((num(x), num(y)), (Some(p), Some(q)) if (p - q).abs() < 1e-4)
            || joint(x) == joint(y) && x.contains("Hash40") && y.contains("Hash40")
    };
    if a.iter().any(|x| x == "--subset") {
        // THE SUBSET PATH HONOURS `--rebased` TOO, and it did not until 2026-09-19.
        //
        // This branch compares effect scripts and `exit`s before the skip/alias logic below ever
        // runs, so a bone the rebase pass legitimately rewrote read as an altered call. It went
        // unnoticed because until Pyra's d-air no ported EFFECT had ever ridden a grafted bone;
        // her `sys_smash_flash` on `sword1` is the first, and the checker failed its own
        // pipeline's correct output.
        //
        // Only the bone is exempt, and only for effect-shaped calls where it is argument 2.
        // Every other argument is still compared exactly, which is the whole value of the check.
        //
        // IT HONOURS `--effect-sub` AS WELL, since 2026-10-17, and for a related reason: the one
        // argument an ADR 0002 substitution changes is the effect ARCHIVE. `tools/effect-subs.tsv`
        // lets a fighter-owned effect that carries information the player needs -- Wolf's f-air arc
        // is the first -- be drawn by a resident `sys_` stand-in at the SOURCE's own joint,
        // offsets, colour and scale. Every one of those still has to match exactly; only the name
        // is allowed to differ, and only for a pair the table actually lists. Without this the
        // checker failed its own pipeline's correct output again, the same way the rebase did.
        //
        // Argument 1 is the archive and argument 2 the joint -- `agent` is 0.
        let arg_eq = |sc: &str, sa: &Vec<String>, oa: &Vec<String>| -> bool {
            if sa == oa {
                return true;
            }
            if let Some(t) = translate(sc, sa) {
                if t.len() == oa.len() && t.iter().zip(oa.iter()).all(|(x, y)| same_arg(x, y)) {
                    return true;
                }
            }
            if sa.len() != oa.len() {
                return false;
            }
            if !(sc.contains("EFFECT") && !sc.contains("LAST_EFFECT_SET")) {
                return false;
            }
            let name = |v: &String| {
                v.trim()
                    .trim_start_matches("Hash40::new(\"")
                    .trim_end_matches("\")")
                    .to_string()
            };
            let swapped = effect_subs
                .iter()
                .any(|(f, t)| name(&sa[1]) == *f && name(&oa[1]) == *t);
            if !swapped && !rebased {
                return false;
            }
            sa.iter()
                .zip(oa.iter())
                .enumerate()
                .all(|(i, (x, y))| (swapped && i == 1) || (rebased && i == 2) || x == y)
        };
        let mut si = 0usize;
        let mut dropped = 0usize;
        for (oc, oa) in &o {
            let found = loop {
                let Some((sc, sa)) = s.get(si) else { break false };
                si += 1;
                if sc == oc && arg_eq(sc, sa, oa) {
                    break true;
                }
                dropped += 1;
            };
            if !found {
                println!("  ours has {oc} that the source does not, or with different arguments");
                println!("    {}", oa.join(", "));
                bad += 1;
                si = 0;
            } else {
                checked += oa.len();
            }
        }
        dropped += s.len().saturating_sub(si);
        println!(
            "  {bad} altered, {} kept and identical, {dropped} dropped of {} source call(s)",
            o.len() - bad,
            s.len()
        );
        std::process::exit(if bad > 0 { 1 } else { 0 });
    }

    // COUNT FIRST. A dropped call is the failure this exists to catch, and comparing pairwise
    // after a drop would report every later call as wrong -- an avalanche that buries the one
    // line that matters.
    if s.len() != o.len() {
        println!("  CALL COUNT {} in source, {} in ours", s.len(), o.len());
        let names = |v: &[(&'static str, Vec<String>)]| {
            v.iter().map(|(c, _)| c.trim_start_matches("macros::").trim_end_matches('(')).collect::<Vec<_>>().join(" ")
        };
        println!("    source: {}", names(&s));
        println!("    ours:   {}", names(&o));
        std::process::exit(1);
    }

    for (i, ((sc, sa), (oc, oa))) in s.iter().zip(o.iter()).enumerate() {
        if sc != oc {
            println!("  call {i} is {} in source and {} in ours", sc, oc);
            bad += 1;
            continue;
        }
        let mut skip: HashSet<usize> = CALLS
            .iter()
            .find(|(c, _)| c == sc)
            .map(|(_, g)| g.iter().copied().collect())
            .unwrap_or_default();
        // `--rebased` ALSO SKIPS THE BONE ANCHOR, for the two scripts `tools/reach rebase`
        // rewrites.
        //
        // The bone is argument 3 of ATTACK and 2 of CATCH, and it is deliberately NOT geometry in
        // the general case: a ported hitbox on the wrong bone is one of the most damaging errors
        // possible and this check is the only thing that would catch it. But Chrom's jab and
        // Palutena's n-air ride grafted bones that do not exist in the runtime skeleton -- proved
        // by `telemetry::mirror_probe`, which reads them nowhere near their own parent and
        // differently in each facing -- so the rebase pass moves them onto `haver` with the
        // offsets rotated by the grafted bone's own transform.
        //
        // Which scripts those are is not declared here: `check-acmd-values.sh` reads the list the
        // rebase pass itself wrote. Every other argument of those calls is still compared, which
        // is the point -- their damage, angle and knockback must still be the source's.
        if rebased {
            match *sc {
                "macros::ATTACK(" => {
                    skip.insert(3);
                }
                "macros::CATCH(" => {
                    skip.insert(2);
                }
                _ => {}
            }
        }
        if sa.len() != oa.len() {
            println!("  call {i} {sc} has {} args in source and {} in ours", sa.len(), oa.len());
            bad += 1;
            continue;
        }
        for (n, (sv, ov)) in sa.iter().zip(oa.iter()).enumerate() {
            if skip.contains(&n) {
                continue;
            }
            checked += 1;
            // A BONE ALIAS IS A NAMED EQUIVALENCE, NOT A SKIPPED ARGUMENT.
            //
            // `--rebased` above drops the bone check entirely, which is the right shape when the
            // offsets were rotated onto a different bone and the anchor is no longer comparable.
            // A REALISED bone is a different situation: `tools/add-bones.sh` reparents the vanilla
            // `HaveL` under `HaveR` and gives it Palutena's `Stick` bind transform, and
            // `rename-stick-track.sh` moves Stick's animation track onto it, so `havel` occupies
            // Stick's frame exactly and the offsets are UNCHANGED. The anchor is still fully
            // comparable -- it just has another name.
            //
            // So this accepts exactly the pairs it was told about and nothing else. Anchoring a
            // ported hitbox to the wrong bone is one of the most damaging errors possible and this
            // check is the only thing that would catch it, so the exemption is one substitution
            // wide rather than one argument wide. `check-acmd-values.sh` derives the pairs from
            // merge-weapons.sh's `realise` column, so they cannot drift from what the merge did.
            if sv != ov && !aliased(sv, ov, &aliases) {
                println!("  call {i} {sc} arg {n}: source {sv} vs ours {ov}");
                bad += 1;
            }
        }
    }

    println!("  {bad} wrong of {checked} non-geometric arg(s) across {} call(s)", s.len());
    if bad > 0 {
        std::process::exit(1);
    }
}

/// True when `sv` and `ov` differ ONLY by a declared bone realisation.
///
/// Both sides are compared as whole argument strings, so this matches
/// `Hash40::new("stick")` against `Hash40::new("havel")` and nothing looser -- a substring test
/// would also accept `Hash40::new("stick2")`, and a bone one character off is exactly the error
/// this check exists to catch.
fn aliased(sv: &str, ov: &str, aliases: &[(String, String)]) -> bool {
    aliases.iter().any(|(from, to)| {
        sv == format!("Hash40::new(\"{from}\")") && ov == format!("Hash40::new(\"{to}\")")
    })
}
