// Recursively dumps a .prc with hash names resolved, so nested tuning tables can be read.
//
// Written to recover Incineroar's Revenge numbers. The `special_lw_revenge_*` labels exist in
// ParamLabels.csv but every one is ABSENT from the common fighter_param.prc -- per-fighter move
// tuning lives in fighter/<name>/param/vl.prc, under `revenge_data` and `param_private`. Reading
// them beats inventing a counter formula and calling it a port.
//
// usage: prcdump <file.prc> <ParamLabels.csv> [path.to.node] [--depth N]
use prc::{ParamKind, ParamStruct};
use prc::hash40::hash40;
use std::collections::HashMap;

fn names_map(labels: &str) -> HashMap<u64, String> {
    let mut m = HashMap::new();
    if let Ok(t) = std::fs::read_to_string(labels) {
        for l in t.lines() {
            let n = l.split(',').last().unwrap_or(l).trim();
            if !n.is_empty() { m.insert(hash40(n).0, n.to_string()); }
        }
    }
    m
}

fn name_of(h: u64, names: &HashMap<u64, String>) -> String {
    names.get(&h).cloned().unwrap_or_else(|| format!("{h:#014x}"))
}

fn dump(v: &ParamKind, names: &HashMap<u64, String>, indent: usize, depth: i32, label: &str) {
    let pad = " ".repeat(indent);
    match v {
        ParamKind::Struct(s) => {
            println!("{pad}{label} struct[{}]", s.0.len());
            if depth == 0 { return }
            for (h, c) in s.0.iter() {
                dump(c, names, indent + 2, depth - 1, &name_of(h.0, names));
            }
        }
        ParamKind::List(l) => {
            println!("{pad}{label} list[{}]", l.0.len());
            if depth == 0 { return }
            for (i, c) in l.0.iter().enumerate() {
                dump(c, names, indent + 2, depth - 1, &format!("[{i}]"));
            }
        }
        ParamKind::Float(f) => println!("{pad}{label} = {f}"),
        ParamKind::I32(i) => println!("{pad}{label} = {i}"),
        ParamKind::U32(u) => println!("{pad}{label} = {u}"),
        ParamKind::I8(i) => println!("{pad}{label} = {i}"),
        ParamKind::U8(u) => println!("{pad}{label} = {u}"),
        ParamKind::I16(i) => println!("{pad}{label} = {i}"),
        ParamKind::U16(u) => println!("{pad}{label} = {u}"),
        ParamKind::Bool(b) => println!("{pad}{label} = {b}"),
        ParamKind::Hash(h) => println!("{pad}{label} = {}", name_of(h.0, names)),
        ParamKind::Str(s) => println!("{pad}{label} = {s:?}"),
    }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("prc");
    let labels = a.next().expect("labels");
    let rest: Vec<String> = a.collect();
    let depth: i32 = rest.iter().position(|x| x == "--depth")
        .and_then(|i| rest.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(4);
    let node = rest.iter().find(|x| !x.starts_with("--") && x.parse::<i32>().is_err()).cloned();

    let names = names_map(&labels);
    let root: ParamStruct = prc::open(&path).expect("parse");

    let mut cur: ParamKind = ParamKind::Struct(root);
    let mut label = "root".to_string();
    if let Some(p) = node {
        for seg in p.split('.') {
            let next = match &cur {
                ParamKind::Struct(s) => s.0.iter().find(|(h, _)| *h == hash40(seg)).map(|(_, v)| v.clone()),
                ParamKind::List(l) => seg.parse::<usize>().ok().and_then(|i| l.0.get(i).cloned()),
                _ => None,
            };
            match next {
                Some(v) => { cur = v; label = seg.to_string() }
                None => { eprintln!("no node '{seg}' under '{label}'"); std::process::exit(1) }
            }
        }
    }
    dump(&cur, &names, 0, depth, &label);
}
