// Parses a shipped .prcxml the way ARCropolis does, and asserts the intended fighter's values
// are actually in it.
//
// The previous check only confirmed a UTF-16 byte order mark was present -- which was itself the
// bug. ARCropolis parses prcxml with `prcx` plus `xml-rs` 0.8, both UTF-8 only, so the file never
// parsed and was silently dropped: no error, no conflict report, and every attribute stayed
// vanilla on hardware. A check that inspects encoding cannot catch that; one that parses can.
//
// usage: prcxmlcheck <file.prcxml> <index> <hash=value>... [--table <name>]
//
// --table defaults to fighter_param_table. fighter_param_motion.prcxml uses
// fighter_param_motion_table, and hardcoding the name meant this gate could not cover the dodge
// params at all.
use prc::{ParamKind, ParamStruct};
use prc::hash40::hash40;
use std::io::BufReader;

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("prcxml path");
    let want_index: usize = a.next().expect("index").parse().expect("index must be a number");
    let rest: Vec<String> = a.collect();
    let table_name = rest.iter().position(|x| x == "--table")
        .and_then(|i| rest.get(i + 1).cloned())
        .unwrap_or_else(|| "fighter_param_table".to_string());
    let specs: Vec<String> = {
        let mut v = vec![];
        let mut it = rest.iter();
        while let Some(x) = it.next() {
            if x == "--table" { it.next(); continue }
            v.push(x.clone());
        }
        v
    };

    let bytes = std::fs::read(&path).expect("read");
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        println!("FAIL {path}: UTF-16 byte order mark -- ARCropolis parses UTF-8 only and will drop this file");
        std::process::exit(1);
    }
    if std::str::from_utf8(&bytes).is_err() {
        println!("FAIL {path}: not valid UTF-8");
        std::process::exit(1);
    }

    let mut r = BufReader::new(&bytes[..]);
    let root: ParamStruct = match prc::xml::read_xml(&mut r) {
        Ok(p) => p,
        Err(e) => { println!("FAIL {path}: does not parse as prcxml: {e:?}"); std::process::exit(1) }
    };

    let Some((_, table)) = root.0.iter().find(|(k, _)| *k == hash40(&table_name)) else {
        println!("FAIL {path}: no {table_name}"); std::process::exit(1)
    };
    let ParamKind::List(list) = table else {
        println!("FAIL {path}: {table_name} is not a list"); std::process::exit(1)
    };
    let Some(ParamKind::Struct(s)) = list.0.get(want_index) else {
        println!("FAIL {path}: index {want_index} is not a struct -- the patch targets the wrong slot");
        std::process::exit(1)
    };

    let mut bad = 0;
    let mut n = 0;
    for spec in specs {
        let Some((k, v)) = spec.split_once('=') else { continue };
        let want: f32 = v.parse().expect("value");
        n += 1;
        let got = s.0.iter().find(|(h, _)| *h == hash40(k)).and_then(|(_, p)| match p {
            ParamKind::Float(f) => Some(*f),
            _ => None,
        });
        match got {
            Some(g) if (g - want).abs() < 1e-4 => {}
            Some(g) => { println!("  FAIL {k}: patch says {g}, expected {want}"); bad += 1 }
            None => { println!("  FAIL {k}: absent from the patch"); bad += 1 }
        }
    }
    if bad == 0 {
        println!("  ok  prcxml parses as UTF-8; index {want_index} carries {} checked attributes ({} total)",
                 n, s.0.len());
    }
    std::process::exit(if bad == 0 { 0 } else { 1 });
}
