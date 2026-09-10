use smash_arc::{ArcFile, ArcLookup, Hash40, FileNode};
use std::collections::HashMap;
fn main() {
    let mut a = std::env::args().skip(1);
    let addr = a.next().unwrap();
    // reverse-hash dictionary so listings are readable
    let mut map: HashMap<u64, String> = HashMap::new();
    for p in ["../../vendor/param-labels/ParamLabels.csv", "../../vendor/Hashes.txt"] {
        if let Ok(txt) = std::fs::read_to_string(p) {
            for line in txt.lines() {
                let n = line.split(',').last().unwrap_or(line).trim();
                if !n.is_empty() { map.insert(Hash40::from(n).0, n.to_string()); }
            }
        }
    }
    let arc = ArcFile::open_over_network(addr.as_str()).expect("open arc");
    for dir in a {
        println!("=== {dir} ===");
        match arc.get_dir_listing(Hash40::from(dir.as_str())) {
            Some(nodes) => for n in nodes {
                let (kind, h) = match n { FileNode::Dir(h) => ("dir ", h.0), FileNode::File(h) => ("file", h.0) };
                println!("  [{kind}] {}", map.get(&h).cloned().unwrap_or_else(|| format!("{h:#014x}")));
            },
            None => println!("  (not found)"),
        }
    }
}
