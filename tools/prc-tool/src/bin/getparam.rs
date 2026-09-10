// Reads named params out of a fighter's own row in fighter_param.prc, resolving the fighter by
// its entry's `fighter_kind` field.
//
// gen-params.sh used to carry hardcoded values with a comment claiming they were read from the
// game. They were not -- they were transcribed, and the same habit put Cloud's scale at 1.03
// against an actual 0.98 and K. Rool's at 1.15 against 0.89. This makes the claim true.
//
// usage: getparam <fighter_param.prc> <ParamLabels.csv> <fighter> <param>...
//   prints "param<TAB>value", or "param<TAB>ABSENT" if the fighter has no such param
use prc::{ParamKind, ParamStruct};
use prc::hash40::hash40;
use std::collections::HashMap;

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("prc");
    let labels = a.next().expect("labels");
    let want_fighter = a.next().expect("fighter");

    let mut names: HashMap<u64, String> = HashMap::new();
    if let Ok(t) = std::fs::read_to_string(&labels) {
        for l in t.lines() {
            let n = l.split(',').last().unwrap_or(l).trim();
            if !n.is_empty() { names.insert(hash40(n).0, n.to_string()); }
        }
    }

    let root: ParamStruct = prc::open(&path).expect("parse");
    let (_, t) = root.0.iter().find(|(k, _)| *k == hash40("fighter_param_table")).expect("table");
    let ParamKind::List(l) = t else { panic!("not a list") };
    let kind_h = hash40("fighter_kind");

    let mut found: Option<&ParamStruct> = None;
    for e in l.0.iter() {
        let ParamKind::Struct(s) = e else { continue };
        let Some((_, ParamKind::Hash(k))) = s.0.iter().find(|(h, _)| *h == kind_h) else { continue };
        let full = names.get(&k.0).cloned().unwrap_or_default();
        if full.strip_prefix("fighter_kind_") == Some(want_fighter.as_str()) { found = Some(s); break }
    }
    let Some(s) = found else {
        eprintln!("no fighter_param_table row with fighter_kind_{want_fighter}");
        std::process::exit(1);
    };

    for p in a {
        let h = hash40(&p);
        match s.0.iter().find(|(k, _)| *k == h).map(|(_, v)| v) {
            // The TYPE is printed too. A prcxml patch has to emit the matching element -- writing
            // <float> for an integer param is a silent no-op, which is the same shape of failure
            // as the UTF-16 encoding bug: accepted, ignored, no error anywhere.
            Some(ParamKind::Float(f)) => println!("{p}\t{f}\tfloat"),
            Some(ParamKind::I32(i))   => println!("{p}\t{i}\tint"),
            Some(ParamKind::U32(u))   => println!("{p}\t{u}\tuint"),
            Some(ParamKind::I8(i))    => println!("{p}\t{i}\tsbyte"),
            Some(ParamKind::U8(u))    => println!("{p}\t{u}\tbyte"),
            Some(ParamKind::I16(i))   => println!("{p}\t{i}\tshort"),
            Some(ParamKind::U16(u))   => println!("{p}\t{u}\tushort"),
            Some(ParamKind::Bool(b))  => println!("{p}\t{b}\tbool"),
            Some(_)                   => println!("{p}\tNON_SCALAR\t-"),
            None                      => println!("{p}\tABSENT\t-"),
        }
    }
}
