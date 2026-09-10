use prc::{ParamKind, ParamStruct};
use prc::hash40::hash40;
fn main() {
    let path = std::env::args().nth(1).expect("prc path");
    let root: ParamStruct = prc::open(&path).expect("parse");
    let cands = ["spikeball","poisonbreath","bosspackun","mario","kaminari","dengeki",
                 "dengekidama","cloud","monsterball","vortex","specialupdummy",
                 "gun","banana","barrel","rocketbarrel","common"];
    println!("{} top-level entries", root.0.len());
    for (h, v) in root.0.iter() {
        let name = cands.iter().find(|c| hash40(c) == *h).map(|s| s.to_string())
            .unwrap_or_else(|| format!("{:#014x}", h.0));
        let kind = match v {
            ParamKind::Struct(s) => format!("struct[{}]", s.0.len()),
            ParamKind::List(l) => format!("list[{}]", l.0.len()),
            _ => "scalar".to_string(),
        };
        println!("  {name:<22} {kind}");
    }
}
