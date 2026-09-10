// Maps fighter name -> fighter_param_table index and `scale`, using each entry's own
// `fighter_kind` field rather than an assumed table order.
//
// This exists because guessing the order was wrong: index 19 turned out to be Pichu, not Falco,
// and the scales read out for Little Mac, Palutena and Mythra were all someone else's. The
// `fighter_kind` field resolves to `fighter_kind_<name>`, so the mapping is exact.
//
// usage: fscale <fighter_param.prc> <ParamLabels.csv> [name...]
use prc::{ParamKind, ParamStruct};
use prc::hash40::hash40;
use std::collections::HashMap;

fn getf(s: &ParamStruct, n: &str) -> Option<f32> {
    let h = hash40(n);
    s.0.iter().find(|(k, _)| *k == h).and_then(|(_, v)| match v { ParamKind::Float(f) => Some(*f), _ => None })
}

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().unwrap();
    let labels = a.next().unwrap();

    let mut names: HashMap<u64, String> = HashMap::new();
    if let Ok(t) = std::fs::read_to_string(&labels) {
        for l in t.lines() {
            let n = l.split(',').last().unwrap_or(l).trim();
            if !n.is_empty() { names.insert(hash40(n).0, n.to_string()); }
        }
    }

    let root: ParamStruct = prc::open(&path).unwrap();
    let (_, t) = root.0.iter().find(|(k, _)| *k == hash40("fighter_param_table")).unwrap();
    let ParamKind::List(l) = t else { panic!() };

    let kind_h = hash40("fighter_kind");
    let mut map: HashMap<String, (usize, f32, f32)> = HashMap::new();
    for (i, e) in l.0.iter().enumerate() {
        let ParamKind::Struct(s) = e else { continue };
        let Some((_, ParamKind::Hash(k))) = s.0.iter().find(|(h, _)| *h == kind_h) else { continue };
        let full = names.get(&k.0).cloned().unwrap_or_else(|| format!("{:#014x}", k.0));
        let short = full.strip_prefix("fighter_kind_").unwrap_or(&full).to_string();
        map.insert(short, (i, getf(s, "scale").unwrap_or(f32::NAN), getf(s, "weight").unwrap_or(f32::NAN)));
    }
    println!("{} entries carry a resolvable fighter_kind", map.len());

    let wanted: Vec<String> = a.collect();
    let list: Vec<String> = if wanted.is_empty() {
        let mut v: Vec<String> = map.keys().cloned().collect(); v.sort(); v
    } else { wanted };
    println!("{:<12} {:>5} {:>8} {:>8}", "fighter", "idx", "scale", "weight");
    for n in list {
        match map.get(&n) {
            Some((i, sc, w)) => println!("{n:<12} {i:>5} {sc:>8.4} {w:>8.1}"),
            None => println!("{n:<12}   -- not found"),
        }
    }
}
