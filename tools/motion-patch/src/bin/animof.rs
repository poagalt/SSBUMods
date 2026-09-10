// Prints the .nuanmb filename a given motion entry uses, for a given fighter.
use hash40::hash40;
use std::collections::HashMap;
fn main() {
    let mut a = std::env::args().skip(1);
    let ex = a.next().unwrap();
    let fighter = a.next().unwrap();
    let mut map: HashMap<u64, String> = HashMap::new();
    for p in ["../../vendor/param-labels/ParamLabels.csv", "../../vendor/Hashes.txt"] {
        if let Ok(t) = std::fs::read_to_string(p) {
            for l in t.lines() {
                let n = l.split(',').last().unwrap_or(l).trim();
                if !n.is_empty() { map.insert(hash40(n).0, n.to_string()); }
            }
        }
    }
    let m = motion_lib::open(format!("{ex}/fighter/{fighter}/motion/body/c00/motion_list.bin")).unwrap();
    for name in a {
        let Some(e) = m.list.get(&hash40(&name)) else { println!("{name:<20} (no such motion)"); continue };
        let anims: Vec<String> = e.animations.iter()
            .map(|an| map.get(&an.name.0).cloned().unwrap_or_else(|| format!("{:#014x}", an.name.0)))
            .collect();
        println!("{name:<20} {}", anims.join(", "));
    }
}
