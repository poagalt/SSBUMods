// Reports vanilla-host behaviour that still runs on moves the mod replaces -- the "vacuum" check.
//
// For every motion entry the mod patches, resolve the game_script it names and report whether the
// plugin overrides that script. Anything unoverridden is vanilla Pikachu logic leaking into a
// ported move, which makes test results ambiguous.
use hash40::hash40;
use std::collections::{HashMap, HashSet};

const PATCHED: &[&str] = &[
    "attack_11","attack_12","attack_13","attack_100","attack_100_end",
    "attack_dash","attack_s3_s","attack_s3_hi","attack_s3_lw","attack_hi3","attack_lw3",
    "attack_s4_s","attack_s4_hi","attack_s4_lw","attack_s4_hold",
    "attack_hi4","attack_hi4_hold","attack_lw4","attack_lw4_hold",
    "attack_air_n","attack_air_f","attack_air_b","attack_air_hi","attack_air_lw",
    "landing_air_n","landing_air_f","landing_air_b","landing_air_hi","landing_air_lw",
    "catch","catch_dash","catch_turn","catch_wait","catch_pull","catch_cut","catch_attack",
    "throw_f","throw_b","throw_hi","throw_lw",
    "down_attack_u","down_attack_d","slip_attack","cliff_attack_quick",
    "escape","escape_f","escape_b","escape_air","escape_air_slide",
];

fn main() {
    let ex = std::env::args().nth(1).expect("extracted root");
    let src = std::env::args().nth(2).expect("plugin src dir");

    // names the plugin registers
    let mut registered: HashSet<String> = HashSet::new();
    fn walk(d: &str, out: &mut Vec<std::path::PathBuf>) {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() { walk(p.to_str().unwrap(), out); }
                else if p.extension().map_or(false, |x| x == "rs") { out.push(p); }
            }
        }
    }
    let mut files = vec![];
    walk(&src, &mut files);
    for f in files {
        for line in std::fs::read_to_string(&f).unwrap_or_default().lines() {
            for key in ["game_acmd(\"", "effect_acmd(\""] {
                if let Some(i) = line.find(key) {
                    let rest = &line[i + key.len()..];
                    if let Some(j) = rest.find('"') { registered.insert(rest[..j].to_string()); }
                }
            }
        }
    }

    let mut names: HashMap<u64, String> = HashMap::new();
    for p in [std::env::var("DICT1").unwrap_or_default(), std::env::var("DICT2").unwrap_or_default()] {
        if let Ok(t) = std::fs::read_to_string(&p) {
            for l in t.lines() {
                let n = l.split(',').last().unwrap_or(l).trim();
                if !n.is_empty() { names.insert(hash40(n).0, n.to_string()); }
            }
        }
    }

    let m = motion_lib::open(format!("{ex}/fighter/pikachu/motion/body/c00/motion_list.bin")).unwrap();
    println!("{:<22} {:<26} {:<10} {}", "motion", "game_script it names", "overridden", "move flag");
    let mut leaks = 0;
    for name in PATCHED {
        let Some(e) = m.list.get(&hash40(name)) else { continue };
        // Resolve FORWARD -- hash each registered script name and compare against the motion's
        // game_script. Reverse lookup does not work here: the label dictionaries contain motion
        // names but not `game_*` ACMD names, so every entry looked unresolvable and the tool
        // reported a false clean of zero leaks.
        let ov = registered.iter().any(|r| hash40(r).0 == e.game_script.0);
        let gs = registered.iter().find(|r| hash40(r).0 == e.game_script.0).cloned()
            .or_else(|| names.get(&e.game_script.0).cloned())
            .unwrap_or_else(|| format!("{:#012x} (unresolved)", e.game_script.0));
        if !ov { leaks += 1; }
        println!("{:<22} {:<26} {:<10} {}", name, gs,
                 if ov { "yes" } else { "NO -- LEAK" }, e.flags.r#move);
    }
    println!("\n{leaks} vanilla scripts still running on replaced moves");
}
