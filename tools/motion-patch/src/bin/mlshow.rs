// Dumps a motion entry in full: flags, blend, extra, the animations it names and -- the field
// that keeps mattering -- the SCRIPT names.
//
// WHY THIS EXISTS. `game_script` and `scripts` decide which ACMDs the engine runs for a motion,
// and a SYNTHESISED entry inherits them from whatever template it was cloned from. The game
// derives `effect_`, `sound_` and `expression_` script names from the same base, and this moveset
// registers no expression scripts at all -- so a synthesised entry pointing at one of Pikachu's
// own script bases silently runs HIS expression script under our animation. Pikachu's Quick Attack
// expression scripts call VisibilityModule::set_whole(false), which is how a working up-B can
// still leave him invisible.
//
// Nothing could see those fields before this: dump.rs lists names only, mlparity compares the
// fields it knows must match, and neither prints what a single entry actually contains.
//
// usage: mlshow <motion_list.bin> <motion>...
//        mlshow <motion_list.bin> --all-scripts     every entry whose script base differs from
//                                                   its own name, which is the interesting set
use hash40::hash40;
use std::collections::HashMap;

fn names_map() -> HashMap<u64, String> {
    let mut m = HashMap::new();
    for p in ["vendor/param-labels/ParamLabels.csv", "vendor/Hashes.txt"] {
        if let Ok(t) = std::fs::read_to_string(p) {
            for l in t.lines() {
                let n = l.split(',').last().unwrap_or(l).trim();
                if !n.is_empty() {
                    m.insert(hash40(n).0, n.to_string());
                }
            }
        }
    }
    m
}

/// Script names are NOT in the shipped dictionaries -- neither ParamLabels nor Hashes.txt carries
/// `expression_specialairhi1` -- so every script hash printed as a raw number and the one field
/// that mattered was unreadable. They are perfectly predictable, though: `<kind>_<motion name with
/// underscores removed>`. Generating the four kinds for every motion in the list resolves all of
/// them, and it stays correct as the list changes because it is derived from the list.
fn add_script_names(m: &mut HashMap<u64, String>, motions: &[String]) {
    for motion in motions {
        let base: String = motion.chars().filter(|c| *c != '_').collect();
        for kind in ["game", "effect", "sound", "expression"] {
            let n = format!("{kind}_{base}");
            m.insert(hash40(&n).0, n);
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.is_empty() {
        eprintln!("usage: mlshow <motion_list.bin> <motion>... | <motion_list.bin> --all-scripts");
        std::process::exit(2);
    }
    let ml = motion_lib::open(&a[0]).expect("open motion_list");
    let mut names = names_map();
    // Every motion in THIS list contributes its four script names, so a script hash resolves even
    // though no shipped dictionary contains it.
    let all_motions: Vec<String> = ml.list.keys().filter_map(|h| names.get(&h.0).cloned()).collect();
    add_script_names(&mut names, &all_motions);
    let names = names;
    let name_of = |h: u64| -> String { names.get(&h).cloned().unwrap_or_else(|| format!("{h:#012x}")) };

    // THE SCRIPT BASE THE ENGINE DERIVES from a motion name is the name with its underscores
    // removed: `special_hi` -> `game_specialhi`, `effect_specialhi`, `sound_specialhi`,
    // `expression_specialhi`. An entry naming anything else is running some OTHER motion's
    // scripts, which is legitimate for a vanilla entry the game deliberately shares and is a bug
    // for one we synthesised.
    if a.get(1).map(|s| s == "--all-scripts").unwrap_or(false) {
        let only: Vec<&str> = a[2..].iter().map(|s| s.as_str()).collect();
        let mut wrong = 0usize;
        let mut checked = 0usize;
        for want in &only {
            let h = hash40(want);
            let Some(m) = ml.list.get(&h) else {
                println!("{want:<26} NOT PRESENT");
                wrong += 1;
                continue;
            };
            checked += 1;
            let base: String = want.chars().filter(|c| *c != '_').collect();
            let mut bad: Vec<String> = Vec::new();
            if m.game_script.0 != hash40(&format!("game_{base}")).0 {
                bad.push(format!("game_script={}", name_of(m.game_script.0)));
            }
            // The order of `scripts` is not fixed, so this asks whether the SET is right rather
            // than matching position by position.
            let want_set: Vec<u64> = ["expression", "sound", "effect"]
                .iter()
                .map(|k| hash40(&format!("{k}_{base}")).0)
                .collect();
            for s in &m.scripts {
                if !want_set.contains(&s.0) {
                    bad.push(format!("scripts has {}", name_of(s.0)));
                }
            }
            if bad.is_empty() {
                continue;
            }
            wrong += 1;
            println!("{want:<26} {}", bad.join(", "));
        }
        println!("\n{wrong} of {checked} entry(ies) name another motion's scripts");
        std::process::exit(if wrong > 0 { 1 } else { 0 });
    }

    for want in &a[1..] {
        let h = hash40(want);
        let Some(m) = ml.list.get(&h) else {
            println!("{want}: NOT PRESENT in {}", a[0]);
            continue;
        };
        println!("{want} ({:#012x})", h.0);
        println!("  game_script  {}", name_of(m.game_script.0));
        println!("  scripts      {}", m.scripts.iter().map(|s| name_of(s.0)).collect::<Vec<_>>().join(" "));
        println!("  animations   {}", m.animations.iter()
            .map(|an| format!("{}(unk {})", name_of(an.name.0), an.unk)).collect::<Vec<_>>().join(" "));
        println!("  blend        {}", m.blend_frames);
        match &m.extra {
            Some(e) => println!("  extra        faf {} xlu {}-{} no_stop_intp {}",
                                e.cancel_frame, e.xlu_start, e.xlu_end, e.no_stop_intp),
            None => println!("  extra        (none)"),
        }
        println!("  flags        {:?}", m.flags);
    }
}
