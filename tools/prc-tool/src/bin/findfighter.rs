// Finds which fighter_param_table index a fighter occupies, by matching the attribute values
// telemetry read off the live game. Guessing the index is how a prcxml patch silently lands on
// the wrong character: every value stays vanilla on the intended fighter and nothing errors.
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
    println!("fighter_param_table has {} entries", list.0.len());

    // Vanilla Pikachu, as measured on hardware by the plugin's dump_params.
    let want: &[(&str, f32)] = &[
        ("weight", 79.0), ("walk_speed_max", 1.302), ("dash_speed", 1.98),
        ("run_speed_max", 2.039), ("jump_y", 35.5), ("air_accel_y", 0.095),
    ];
    let mut hits = vec![];
    for (i, e) in list.0.iter().enumerate() {
        let ParamKind::Struct(s) = e else { continue };
        let n = want.iter().filter(|(k, v)| getf(s, k).map_or(false, |g| (g - v).abs() < 0.002)).count();
        if n == want.len() { hits.push(i) }
    }
    println!("indices matching ALL {} live Pikachu values: {:?}", want.len(), hits);
    for i in [7usize].iter().chain(hits.iter()) {
        if let Some(ParamKind::Struct(s)) = list.0.get(*i) {
            println!("  index {i}: weight={:?} walk={:?} dash={:?} jump_y={:?}",
                     getf(s, "weight"), getf(s, "walk_speed_max"),
                     getf(s, "dash_speed"), getf(s, "jump_y"));
        }
    }
}
