// Appends ONE entry of a donor's top-level list to the host's list of the same name.
//
// WHY THIS EXISTS. vlmerge copies or replaces a whole top-level object, which is wrong for lists the
// host already has and must keep: Pikachu's `cliff_hang_data` has one entry, his own default ledge
// box, and Alolan Whip needs Incineroar's `cliff_hang_data[1]` (FIGHTER_GAOGAEN_CLIFF_HANG_DATA_
// SPECIAL_S) beside it so `GroundModule::select_cliff_hangdata(boma, 1)` selects the whip's box
// instead of indexing past the end of Pikachu's table. Replacing the list would also replace his
// ordinary ledge grab with Incineroar's.
//
// THE INDEX IS ASSERTED, NOT ASSUMED. The engine selects by index, and the index is the source's
// constant, so the entry has to land at exactly that position. `<at>` is the index the caller
// needs; this refuses unless the host list currently has exactly `<at>` entries.
//
// --scale F multiplies every float field of the copied entry. cliff_hang_data is measured from the
// fighter's origin, the same as a `top`-anchored hitbox offset, so it takes the same
// source_scale / 0.96 correction (tools/reach/src/bin/topscale.rs) -- under the same hypothesis
// that the engine multiplies it by the fighter's scale, which nothing here has measured.
//
// usage: vlappend <host vl.prc> <donor vl.prc> <list> <donor index> <at> <out vl.prc> [--scale F]
use hash40::hash40;
use prc::{ParamKind, ParamStruct};

fn scale_floats(v: &mut ParamKind, f: f32, n: &mut usize) {
    match v {
        ParamKind::Float(x) => {
            *x *= f;
            *n += 1;
        }
        ParamKind::Struct(s) => s.0.iter_mut().for_each(|(_, v)| scale_floats(v, f, n)),
        ParamKind::List(l) => l.0.iter_mut().for_each(|v| scale_floats(v, f, n)),
        _ => {}
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let scale = args.iter().position(|a| a == "--scale").map(|i| {
        args.get(i + 1).and_then(|s| s.parse::<f32>().ok()).expect("--scale needs a number")
    });
    let pos: Vec<&String> = {
        let mut skip = false;
        args.iter()
            .filter(|a| {
                if skip { skip = false; return false; }
                if *a == "--scale" { skip = true; return false; }
                true
            })
            .collect()
    };
    if pos.len() != 6 {
        eprintln!("usage: vlappend <host vl.prc> <donor vl.prc> <list> <donor index> <at> <out vl.prc> [--scale F]");
        std::process::exit(2);
    }
    let (host_p, donor_p, name, out_p) = (pos[0], pos[1], pos[2], pos[5]);
    let donor_idx: usize = pos[3].parse().expect("donor index");
    let at: usize = pos[4].parse().expect("at");

    let mut host: ParamStruct = prc::open(host_p).expect("parse host");
    let donor: ParamStruct = prc::open(donor_p).expect("parse donor");
    let key = hash40(name);

    let Some((_, ParamKind::List(dlist))) = donor.0.iter().find(|(h, _)| *h == key) else {
        eprintln!("donor has no list {name}");
        std::process::exit(1);
    };
    let Some(entry) = dlist.0.get(donor_idx) else {
        eprintln!("donor {name} has {} entries; no [{donor_idx}]", dlist.0.len());
        std::process::exit(1);
    };
    let mut entry = entry.clone();
    let mut scaled = 0;
    if let Some(f) = scale {
        scale_floats(&mut entry, f, &mut scaled);
    }

    let Some((_, ParamKind::List(hlist))) = host.0.iter_mut().find(|(h, _)| *h == key) else {
        eprintln!("host has no list {name}; this tool only appends to a list the host already has");
        std::process::exit(1);
    };
    if hlist.0.len() != at {
        eprintln!("host {name} has {} entries; appending would land at [{}], not the [{at}] asked for",
                  hlist.0.len(), hlist.0.len());
        std::process::exit(1);
    }
    hlist.0.push(entry);
    println!("appended donor {name}[{donor_idx}] as host {name}[{at}]{}",
             scale.map_or(String::new(), |f| format!(", {scaled} float(s) scaled by {f}")));
    prc::save(out_p, &host).expect("save");
    println!("wrote {out_p}");
}
