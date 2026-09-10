// Prints every field name in one fighter_param_table entry, looking for something that identifies
// which fighter it is. Guessing the table order produced wrong scales, so the mapping needs a
// source of truth rather than an assumed ordering.
use prc::{ParamKind, ParamStruct};
use prc::hash40::hash40;
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let idx: usize = std::env::args().nth(2).unwrap().parse().unwrap();
    let root: ParamStruct = prc::open(&path).unwrap();
    let (_, t) = root.0.iter().find(|(k, _)| *k == hash40("fighter_param_table")).unwrap();
    let ParamKind::List(l) = t else { panic!() };
    let Some(ParamKind::Struct(s)) = l.0.get(idx) else { panic!("no struct at {idx}") };
    println!("index {idx}: {} fields", s.0.len());
    for (h, v) in s.0.iter() {
        let kind = match v {
            ParamKind::Hash(x) => format!("hash40 {:#014x}", x.0),
            ParamKind::Str(x) => format!("string {x:?}"),
            ParamKind::I32(x) => format!("i32 {x}"),
            ParamKind::U32(x) => format!("u32 {x}"),
            ParamKind::Float(_) => continue,
            _ => "other".into(),
        };
        println!("  {:#014x}  {}", h.0, kind);
    }
}
