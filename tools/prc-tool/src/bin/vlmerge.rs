// Copies one top-level param object from a donor fighter's vl.prc into a host's vl.prc.
//
// smashline's cloning code builds its allowed-name list as `param_<new_article_name>` and reads
// it out of the HOST fighter's own vl.prc (src/params.rs, init_fighter_p_object). If the host
// has no such entry the lookup yields nothing and the game null-derefs during fighter init.
//
// usage: vlmerge <host vl.prc> <donor vl.prc> <object name> <out vl.prc>
use prc::{ParamKind, ParamStruct};
use prc::hash40::hash40;

fn main() {
    let mut a = std::env::args().skip(1);
    let host_p = a.next().expect("host vl.prc");
    let donor_p = a.next().expect("donor vl.prc");
    let name = a.next().expect("object name, e.g. param_spikeball");
    let out_p = a.next().expect("output path");

    let mut host: ParamStruct = prc::open(&host_p).expect("parse host");
    let donor: ParamStruct = prc::open(&donor_p).expect("parse donor");
    let key = hash40(&name);

    if host.0.iter().any(|(h, _)| *h == key) {
        println!("host already has {name}; nothing to do");
        return;
    }
    let Some((_, val)) = donor.0.iter().find(|(h, _)| *h == key) else {
        eprintln!("donor has no {name} ({:#014x})", key.0);
        std::process::exit(1);
    };

    let describe = |v: &ParamKind| match v {
        ParamKind::Struct(s) => format!("struct[{}]", s.0.len()),
        ParamKind::List(l) => format!("list[{}]", l.0.len()),
        _ => "scalar".into(),
    };
    println!("copying {name} ({:#014x}) {}", key.0, describe(val));

    host.0.push((key, val.clone()));
    println!("host now has {} top-level entries", host.0.len());
    prc::save(&out_p, &host).expect("save");
    println!("wrote {out_p}");
}
