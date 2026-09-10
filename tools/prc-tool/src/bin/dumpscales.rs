// Dumps index, weight, scale and a couple of movement stats for every fighter_param_table entry,
// so a fighter can be identified by its published stats rather than by guessing the table order.
use prc::{ParamKind, ParamStruct};
use prc::hash40::hash40;
fn getf(s: &ParamStruct, n: &str) -> f32 {
    let h = hash40(n);
    s.0.iter().find(|(k, _)| *k == h).and_then(|(_, v)| match v { ParamKind::Float(f) => Some(*f), _ => None }).unwrap_or(f32::NAN)
}
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let root: ParamStruct = prc::open(&path).unwrap();
    let (_, t) = root.0.iter().find(|(k, _)| *k == hash40("fighter_param_table")).unwrap();
    let ParamKind::List(l) = t else { panic!() };
    println!("{:>4} {:>7} {:>7} {:>7} {:>7} {:>7}", "idx", "weight", "scale", "walk", "run", "jump_y");
    for (i, e) in l.0.iter().enumerate() {
        let ParamKind::Struct(s) = e else { continue };
        println!("{:>4} {:>7.1} {:>7.4} {:>7.3} {:>7.3} {:>7.2}", i,
                 getf(s,"weight"), getf(s,"scale"), getf(s,"walk_speed_max"),
                 getf(s,"run_speed_max"), getf(s,"jump_y"));
    }
}
