// Reads named params out of fighter_param_motion.prc for a given fighter.
//
// This file holds what fighter_param.prc does not: getparam reports escape_air_slide_speed,
// escape_air_slide_distance, escape_air_cancel_frame and every other escape_* param ABSENT from
// fighter_param.prc for both Pikachu and Fox. The dodge port has been missing them since v0.2.
//
// Its structure is not the fighter_param_table shape, so this dumps the top level first when
// asked, rather than assuming a layout.
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

fn kindstr(v: &ParamKind) -> String {
    match v {
        ParamKind::Float(f) => format!("{f}\tfloat"),
        ParamKind::I32(i) => format!("{i}\tint"),
        ParamKind::U32(u) => format!("{u}\tuint"),
        ParamKind::I8(i) => format!("{i}\tsbyte"),
        ParamKind::U8(u) => format!("{u}\tbyte"),
        ParamKind::I16(i) => format!("{i}\tshort"),
        ParamKind::U16(u) => format!("{u}\tushort"),
        ParamKind::Bool(b) => format!("{b}\tbool"),
        ParamKind::Hash(h) => format!("{:#014x}\thash40", h.0),
        ParamKind::Str(s) => format!("{s}\tstring"),
        ParamKind::Struct(s) => format!("struct[{}]\t-", s.0.len()),
        ParamKind::List(l) => format!("list[{}]\t-", l.0.len()),
    }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("fighter_param_motion.prc");
    let labels = a.next().expect("ParamLabels.csv");
    let names = names_map(&labels);
    let root: ParamStruct = prc::open(&path).expect("parse");

    let mode = a.next().unwrap_or_else(|| "--top".into());
    if mode == "--top" {
        println!("{} top-level entries", root.0.len());
        for (h, v) in root.0.iter() {
            let n = names.get(&h.0).cloned().unwrap_or_else(|| format!("{:#014x}", h.0));
            println!("  {n:<40} {}", kindstr(v));
        }
        return;
    }

    // mode is a fighter name. The file is a fighter_param_motion_table list, the same shape as
    // fighter_param_table, so the row is found by its own `fighter_kind` field rather than by
    // assuming Pikachu keeps index 7 across files -- guessing table order has been wrong before.
    let (_, table) = root.0.iter()
        .find(|(k, _)| *k == hash40("fighter_param_motion_table"))
        .expect("no fighter_param_motion_table");
    let ParamKind::List(list) = table else { panic!("table is not a list") };
    let kind_h = hash40("fighter_kind");
    let mut found: Option<&ParamStruct> = None;
    let mut idx = usize::MAX;
    for (i, e) in list.0.iter().enumerate() {
        let ParamKind::Struct(st) = e else { continue };
        let Some((_, ParamKind::Hash(k))) = st.0.iter().find(|(h, _)| *h == kind_h) else { continue };
        let full = names.get(&k.0).cloned().unwrap_or_default();
        if full.strip_prefix("fighter_kind_") == Some(mode.as_str()) { found = Some(st); idx = i; break }
    }
    let Some(s) = found else {
        eprintln!("no fighter_param_motion_table row with fighter_kind_{mode}");
        std::process::exit(1);
    };
    eprintln!("({mode} is index {idx} of {})", list.0.len());
    let wanted: Vec<String> = a.collect();
    if wanted.is_empty() {
        println!("{} params under {mode}", s.0.len());
        for (h, v) in s.0.iter() {
            let n = names.get(&h.0).cloned().unwrap_or_else(|| format!("{:#014x}", h.0));
            println!("  {n:<40} {}", kindstr(v));
        }
        return;
    }
    for p in wanted {
        let h = hash40(&p);
        match s.0.iter().find(|(k, _)| *k == h).map(|(_, v)| v) {
            Some(v) => println!("{p}\t{}", kindstr(v)),
            None => println!("{p}\tABSENT\t-"),
        }
    }
}
