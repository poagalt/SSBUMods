// Rewrites [rat] telemetry lines, replacing hash40 motion ids with names.
use hash40::hash40;
use std::collections::HashMap;
use std::io::BufRead;
fn main() {
    let mut map: HashMap<u64, String> = HashMap::new();
    for p in [&std::env::var("DICT1").unwrap_or_default(), &std::env::var("DICT2").unwrap_or_default()] {
        if let Ok(t) = std::fs::read_to_string(p) {
            for l in t.lines() {
                let n = l.split(',').last().unwrap_or(l).trim();
                if n.is_empty() { continue; }
                map.insert(hash40(n).0, n.to_string());
            }
        }
    }
    let stdin = std::io::stdin();
    for line in stdin.lock().lines().map_while(Result::ok) {
        let mut out = line.clone();
        let mut i = 0;
        while let Some(p) = line[i..].find("0x") {
            let s = i + p;
            let hex: String = line[s+2..].chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            if hex.len() >= 8 {
                if let Ok(v) = u64::from_str_radix(&hex, 16) {
                    if let Some(n) = map.get(&v) {
                        out = out.replace(&format!("0x{hex}"), &format!("{n:<22}"));
                    }
                }
            }
            i = s + 2;
        }
        println!("{out}");
    }
}
