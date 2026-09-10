// Parity harness: every ACMD script the plugin registers must be reachable.
//
// A registration like .game_acmd("game_attackhi4", ..) only ever fires if some motion entry
// names that script in its `game_script` field. Little Mac's angled f-smashes were silently
// dead for exactly this reason before the motion entries were synthesized.
use hash40::hash40;
use std::collections::HashSet;

fn main() {
    let ml = std::env::args().nth(1).expect("usage: checkacmd <motion_list.bin> <src dir>");
    let src = std::env::args().nth(2).expect("plugin src dir");
    let m = motion_lib::open(&ml).expect("open motion_list");

    let scripts: HashSet<u64> = m.list.values().map(|v| v.game_script.0).collect();

    let mut fail = 0;
    let mut total = 0;
    for entry in walk(&src) {
        let text = std::fs::read_to_string(&entry).unwrap_or_default();
        for line in text.lines() {
            let Some(i) = line.find(".game_acmd(\"") else { continue };
            let rest = &line[i + 12..];
            let Some(j) = rest.find('"') else { continue };
            let name = &rest[..j];
            total += 1;
            if !scripts.contains(&hash40(name).0) {
                println!("UNREACHABLE  {name:<26} no motion entry names it");
                fail += 1;
            }
        }
    }
    println!("\nacmd reachability: {}/{} reachable", total - fail, total);
    if fail > 0 { std::process::exit(1); }
}

fn walk(dir: &str) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() { out.extend(walk(p.to_str().unwrap())); }
            else if p.extension().map_or(false, |x| x == "rs") { out.push(p); }
        }
    }
    out
}
