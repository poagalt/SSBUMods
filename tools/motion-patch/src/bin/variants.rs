// Finds every motion Pikachu has that his move's SOURCE fighter does not, so his own animation
// cannot quietly keep playing on a replaced move.
//
// THE BUG CLASS THIS ENUMERATES. Mythra has no `attack_s3_hi` and no `attack_s3_lw` at all -- her
// f-tilt is one animation for all three angles. Pikachu DOES have c01attacks3hi/lw.nuanmb, so
// dropping in her c01attacks3s left two of the three angles playing his tail whip, and it took a
// playtest report ("pikachu's down and up angled ftilt animations still exist") to notice. That
// was found by accident. There is no reason to believe f-tilt is the only case, and with every
// fighter's motion_list now extracted the question is answerable exhaustively instead.
//
// HOW IT DECIDES. For each replaced slot, take the family of motions Pikachu has whose names extend
// the slot's base name (attack_s3_s -> attack_s3_hi, attack_s3_lw, ...) and report any the source
// lacks. A missing one means the game will play PIKACHU's animation for that variant unless
// motion-patch's REANIM table points it somewhere deliberate.
//
// A GAP IS NOT AUTOMATICALLY A BUG, so the report classifies rather than just listing. Two things
// can make one fine, and both are checked rather than assumed:
//
//   REPOINTED  the shipped motion_list names a different animation than vanilla Pikachu does, so
//              motion-patch's REANIM table has already dealt with it. Read from the built file, so
//              it cannot claim a fix that did not ship.
//   LOCKED OUT the motion is only reachable through a status the plugin makes bounce straight back
//              to wait/fall -- see src/specials/lockout/mod.rs. Listed explicitly below with the
//              reason, because "nothing requests it" is an argument about absent code and this
//              moveset has been wrong about exactly that twice.
//
// Anything left over is a real gap: Pikachu's own animation will play on a replaced move.
//
// usage: variants <extracted-root> [shipped-motion_list.bin]
use hash40::hash40;
use std::collections::{HashMap, HashSet};

// slot base -> source fighter. Mirrors tools/motion-patch MAP's coverage, by move family rather
// than by exact entry, because the point is to find the entries MAP does not mention.
const FAMILIES: &[(&str, &str)] = &[
    ("attack_11", "chrom"),
    ("attack_dash", "elight"),
    ("attack_s3", "elight"),
    ("attack_hi3", "falco"),
    ("attack_lw3", "robot"),
    ("attack_s4", "littlemac"),
    ("attack_hi4", "fox"),
    ("attack_lw4", "gamewatch"),
    ("attack_air_n", "palutena"),
    ("attack_air_f", "demon"),
    ("attack_air_b", "cloud"),
    ("attack_air_hi", "kamui"),
    ("attack_air_lw", "krool"),
    ("landing_air_n", "palutena"),
    ("landing_air_f", "demon"),
    ("landing_air_b", "cloud"),
    ("landing_air_hi", "kamui"),
    ("landing_air_lw", "krool"),
    ("catch", "palutena"),
    ("throw_f", "donkey"),
    ("throw_b", "gaogaen"),
    ("throw_hi", "mewtwo"),
    ("down_attack", "koopa"),
    ("slip_attack", "koopa"),
    ("cliff_attack", "ganon"),
    ("escape", "fox"),
    ("special_s", "gaogaen"),
    ("special_lw", "gaogaen"),
    ("special_hi", "gamewatch"),
];

// MOTION NAMES ARE IN ParamLabels.csv, NOT Hashes.txt, and the two files have different shapes:
// ParamLabels lines are `0x0b7dd72b57,attack_s3_s` while Hashes.txt is one bare name per line.
// Reading only Hashes.txt resolved almost nothing, which surfaced as every source reporting
// "motion_list not extracted" -- the same wrong-labels bug wearing two different masks. Taking the
// last comma-separated field handles both, which is what dump.rs and animof.rs already do.
fn names() -> HashMap<u64, String> {
    let mut m = HashMap::new();
    for p in [
        "vendor/param-labels/ParamLabels.csv",
        "../../vendor/param-labels/ParamLabels.csv",
        "vendor/Hashes.txt",
        "../../vendor/Hashes.txt",
    ] {
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

// Motions reachable only through a status src/specials/lockout/mod.rs bounces out of. Keep this in
// step with that module's RETIRED list; a name here with no lockout behind it is a false clean.
const LOCKED_OUT: &[(&str, &str)] = &[
    ("special_s",        "Skull Bash release, status 0x1E5"),
    ("special_s_hold",   "Skull Bash charge, status 0x1E3"),
    ("special_s_weak",   "Skull Bash weak release, status 0x1E4"),
    ("special_lw_hit",   "Thunder hit reaction, status 0x1E7"),
    ("special_hi_start", "Quick Attack windup, entered only from SPECIAL_HI which we own"),
    ("special_hi_end",   "Quick Attack recovery, status 0x1E2"),
];

fn entries(extracted: &str, fighter: &str, labels: &HashMap<u64, String>) -> HashSet<String> {
    let p = format!("{extracted}/fighter/{fighter}/motion/body/c00/motion_list.bin");
    let Ok(m) = motion_lib::open(&p) else {
        return HashSet::new();
    };
    m.list
        .keys()
        .filter_map(|k| labels.get(&k.0).cloned())
        .collect()
}

fn main() {
    let ex = std::env::args().nth(1).unwrap_or_else(|| "extracted".into());
    let labels = names();
    let pika = entries(&ex, "pikachu", &labels);
    let vanilla = motion_lib::open(format!("{ex}/fighter/pikachu/motion/body/c00/motion_list.bin"))
        .expect("vanilla pikachu motion_list");
    let shipped = std::env::args().nth(2).and_then(|p| motion_lib::open(p).ok());
    if pika.is_empty() {
        eprintln!("no pikachu motion entries resolved -- is vendor/Hashes.txt reachable?");
        std::process::exit(1);
    }

    let mut cache: HashMap<String, HashSet<String>> = HashMap::new();
    let mut gaps = 0usize;
    let mut locked = 0usize;
    let mut repointed_n = 0usize;
    let mut checked = 0usize;

    println!("{:<26} {:<11} {}", "pikachu motion", "source", "status");
    println!("{}", "-".repeat(72));

    for (base, fighter) in FAMILIES {
        let src = cache
            .entry(fighter.to_string())
            .or_insert_with(|| entries(&ex, fighter, &labels))
            .clone();
        if src.is_empty() {
            println!("{:<26} {:<11} source motion_list not extracted", base, fighter);
            continue;
        }

        // Pikachu's motions in this family: the base itself and anything extending it. Sorted so
        // the report is stable and diffable between runs.
        let mut family: Vec<&String> = pika
            .iter()
            .filter(|n| n.as_str() == *base || n.starts_with(&format!("{base}_")))
            .collect();
        family.sort();

        for m in family {
            checked += 1;
            if src.contains(m) {
                continue;
            }
            // Repointed? Compare what the SHIPPED motion_list names against vanilla Pikachu.
            let key = hash40(m);
            let repointed = match (&shipped, vanilla.list.get(&key)) {
                (Some(s), Some(van)) => s
                    .list
                    .get(&key)
                    .map_or(false, |cur| cur.animations != van.animations),
                _ => false,
            };
            if repointed {
                repointed_n += 1;
                println!("{:<26} {:<11} REPOINTED by motion-patch REANIM", m, fighter);
                continue;
            }
            if let Some((_, why)) = LOCKED_OUT.iter().find(|(n, _)| n == m) {
                locked += 1;
                println!("{:<26} {:<11} LOCKED OUT -- {why}", m, fighter);
                continue;
            }
            gaps += 1;
            println!("{:<26} {:<11} GAP -- pikachu's own animation plays", m, fighter);
        }
    }

    println!();
    println!(
        "{checked} motions checked across {} families: {gaps} gap(s), {repointed_n} repointed, {locked} locked out",
        FAMILIES.len()
    );
    if gaps > 0 {
        println!("each gap needs a deliberate decision: point it at a sibling via motion-patch's");
        println!("REANIM table, or record why keeping Pikachu's animation is correct.");
    }
}
