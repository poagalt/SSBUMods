use prc::{ParamKind, ParamStruct};
use prc::hash40::{hash40, Hash40};
use std::collections::HashMap;

fn labels() -> HashMap<Hash40, String> {
    let mut m = HashMap::new();
    if let Ok(txt) = std::fs::read_to_string("../../vendor/param-labels/ParamLabels.csv") {
        for line in txt.lines() {
            if let Some((h, n)) = line.split_once(',') {
                if let Ok(v) = u64::from_str_radix(h.trim_start_matches("0x"), 16) {
                    m.insert(Hash40(v), n.to_string());
                }
            }
        }
    }
    m
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: prc-tool <prc> <fighter_kind> [keys...]");
    let want_kind = std::env::args().nth(2).unwrap_or_else(|| "fighter_kind_pikachu".into());
    let keys: Vec<String> = std::env::args().skip(3).collect();
    let lm = labels();
    let want = hash40(&want_kind);
    let root: ParamStruct = prc::open(&path).expect("failed to parse prc");
    let table_hash = hash40("fighter_param_table");
    let kind_hash = hash40("fighter_kind");

    for (h, v) in root.0.iter() {
        if *h != table_hash { continue; }
        if let ParamKind::List(list) = v {
            for (i, entry) in list.0.iter().enumerate() {
                let ParamKind::Struct(s) = entry else { continue };
                let is_match = s.0.iter().any(|(fh, fv)| {
                    *fh == kind_hash && matches!(fv, ParamKind::Hash(hv) if *hv == want)
                });
                if !is_match { continue; }
                println!("# {} -> index {}", want_kind, i);
                for (fh, fv) in s.0.iter() {
                    let name = lm.get(fh).cloned().unwrap_or_else(|| format!("{:#012x}", fh.0));
                    if !keys.is_empty() && !keys.iter().any(|k| k == &name) { continue; }
                    let val = match fv {
                        ParamKind::Float(f) => format!("{}", f),
                        ParamKind::I32(n) => format!("{}", n),
                        ParamKind::U32(n) => format!("{}", n),
                        ParamKind::Bool(b) => format!("{}", b),
                        ParamKind::Hash(h2) => lm.get(h2).cloned().unwrap_or_else(|| format!("{:#012x}", h2.0)),
                        other => format!("{:?}", other),
                    };
                    println!("{:<28} {}", name, val);
                }
            }
        }
    }
}
