use hash40::hash40;
use std::collections::HashMap;
fn main() {
    let mut a = std::env::args().skip(1);
    let ex = a.next().unwrap();
    let fighter = a.next().unwrap();
    let filter = a.next().unwrap_or_default();

    let mut map: HashMap<u64, String> = HashMap::new();
    for p in ["../../vendor/param-labels/ParamLabels.csv", "../../vendor/Hashes.txt"] {
        if let Ok(txt) = std::fs::read_to_string(p) {
            for line in txt.lines() {
                let name = line.split(',').last().unwrap_or(line).trim();
                if name.is_empty() { continue; }
                map.insert(hash40(name).0, name.to_string());
            }
        }
    }
    let m = motion_lib::open(format!("{ex}/fighter/{fighter}/motion/body/c00/motion_list.bin")).unwrap();
    let mut names: Vec<String> = m.list.keys()
        .map(|k| map.get(&k.0).cloned().unwrap_or_else(|| format!("{:#012x}", k.0)))
        .collect();
    names.sort();
    for n in names {
        if filter.is_empty() || n.contains(&filter) { println!("{n}"); }
    }
}
