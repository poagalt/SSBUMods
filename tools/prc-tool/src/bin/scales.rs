// Reads each fighter's `scale` and `weight` out of fighter_param_table by index.
//
// The table is indexed by the classic fighter order, not by name, so the mapping has to be
// verified rather than assumed. This prints the anchors whose scale is already known from earlier
// measurement (Fox 1.08, Falco 1.05, Little Mac 1.11, Palutena 0.95, Mythra 1.112, and Pikachu
// 0.96 at the index findfighter.rs proved) alongside the fighters we still need. If the anchors
// match, the indices for the rest are trustworthy.
use prc::{ParamKind, ParamStruct};
use prc::hash40::hash40;

fn getf(s: &ParamStruct, name: &str) -> Option<f32> {
    let h = hash40(name);
    s.0.iter().find(|(k, _)| *k == h).and_then(|(_, v)| match v {
        ParamKind::Float(f) => Some(*f),
        _ => None,
    })
}

fn main() {
    let path = std::env::args().nth(1).expect("fighter_param.prc");
    let root: ParamStruct = prc::open(&path).expect("parse");
    let (_, table) = root.0.iter().find(|(k, _)| *k == hash40("fighter_param_table")).expect("table");
    let ParamKind::List(list) = table else { panic!("not a list") };

    // (index, name, expected scale or None, expected weight or None)
    let probes: &[(usize, &str, Option<f32>, Option<f32>)] = &[
        (6,  "fox        ANCHOR", Some(1.08),  None),
        (7,  "pikachu    ANCHOR", Some(0.96),  Some(79.0)),
        (19, "falco      ANCHOR", Some(1.05),  None),
        (48, "littlemac  ANCHOR", Some(1.11),  None),
        (50, "palutena   ANCHOR", Some(0.95),  None),
        (76, "elight     ANCHOR", Some(1.112), None),
        (13, "koopa      NEED",   None, Some(135.0)),
        (22, "ganon      NEED",   None, Some(118.0)),
        (23, "mewtwo     NEED",   None, Some(79.0)),
        (25, "gamewatch  NEED",   None, Some(75.0)),
        (41, "robot      NEED",   None, Some(106.0)),
        (65, "gaogaen    NEED",   None, Some(116.0)),
    ];

    let mut bad = 0;
    println!("{:>5}  {:<18} {:>7} {:>8}  {}", "idx", "fighter", "scale", "weight", "verdict");
    for (i, name, exp_scale, exp_weight) in probes {
        let Some(ParamKind::Struct(s)) = list.0.get(*i) else {
            println!("{i:>5}  {name:<18} -- not a struct"); bad += 1; continue };
        let sc = getf(s, "scale").unwrap_or(f32::NAN);
        let w = getf(s, "weight").unwrap_or(f32::NAN);
        let mut verdict = String::new();
        if let Some(e) = exp_scale {
            if (sc - e).abs() < 5e-4 { verdict.push_str("scale ok  ") }
            else { verdict.push_str(&format!("SCALE MISMATCH (want {e})  ")); bad += 1 }
        }
        if let Some(e) = exp_weight {
            if (w - e).abs() < 0.5 { verdict.push_str("weight ok") }
            else { verdict.push_str(&format!("WEIGHT MISMATCH (want {e})")); bad += 1 }
        }
        println!("{i:>5}  {name:<18} {sc:>7.4} {w:>8.1}  {verdict}");
    }
    println!("\n{} mismatch(es)", bad);
    std::process::exit(if bad == 0 { 0 } else { 1 });
}
