// Copies one top-level param object from a donor fighter's vl.prc into a host's vl.prc.
//
// TWO REASONS TO DO THIS, and they need different behaviour when the host already has the object.
//
// 1. ARTICLE PARAMS, the original reason. smashline's cloning code builds its allowed-name list as
//    `param_<new_article_name>` and reads it out of the HOST fighter's own vl.prc (src/params.rs,
//    init_fighter_p_object). If the host has no such entry the lookup yields nothing and the game
//    null-derefs during fighter init. The host never has one, so "skip if present" is right.
//
// 2. MOVE PARAMS THE ENGINE READS BACK OFF THE HOST. Alolan Whip's victim runs common engine code
//    -- FIGHTER_STATUS_KIND_SWING_GAOGAEN_THROWN, _ATTACH_ROPE, _RETURN -- and that code reads
//    `special_s_through_target_speed` and friends from the GRABBER. Pikachu already HAS a
//    `param_special_s`: Skull Bash's, which has none of those keys, so every lookup returns 0 and
//    the whipped victim never travels. Here the object must be REPLACED, not skipped.
//
// --replace makes the difference explicit rather than guessing from whether the key is there.
//
// usage: vlmerge [--replace] <host vl.prc> <donor vl.prc> <object name> <out vl.prc>
use prc::{ParamKind, ParamStruct};
use prc::hash40::hash40;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let replace = args.iter().any(|a| a == "--replace");
    args.retain(|a| a != "--replace");
    let mut a = args.into_iter();
    let host_p = a.next().expect("host vl.prc");
    let donor_p = a.next().expect("donor vl.prc");
    let name = a.next().expect("object name, e.g. param_spikeball");
    let out_p = a.next().expect("output path");

    let mut host: ParamStruct = prc::open(&host_p).expect("parse host");
    let donor: ParamStruct = prc::open(&donor_p).expect("parse donor");
    let key = hash40(&name);

    let existing = host.0.iter().position(|(h, _)| *h == key);
    if existing.is_some() && !replace {
        println!("host already has {name}; nothing to do (pass --replace to overwrite)");
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

    match existing {
        // Overwrite in place, keeping the object's position in the struct. The order of a prc's
        // top-level entries is not documented as significant, and not disturbing it is free.
        Some(i) => {
            println!(
                "replacing {name} ({:#014x}) {} -> {}",
                key.0, describe(&host.0[i].1), describe(val)
            );
            host.0[i].1 = val.clone();
        }
        None => {
            println!("copying {name} ({:#014x}) {}", key.0, describe(val));
            host.0.push((key, val.clone()));
        }
    }
    println!("host now has {} top-level entries", host.0.len());
    prc::save(&out_p, &host).expect("save");
    println!("wrote {out_p}");
}
