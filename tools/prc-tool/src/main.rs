use prc::{ParamKind, ParamStruct};
use prc::hash40::{hash40, Hash40};
use std::collections::HashMap;

fn labels() -> HashMap<Hash40, String> {
    let mut m = HashMap::new();
    if let Ok(txt) = std::fs::read_to_string("../../vendor/param-labels/ParamLabels.csv") {
        for line in txt.lines() {
            if let Some((h, n)) = line.split_once(',') {
                if let Ok(v) = u64::from_str_radix(h.trim_start_matches("0x"), 16) {
                    m.insert(Hash40(v), n.to_string());
                }
            }
        }
    }
    m
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: prc-tool <prc> <fighter_kind> [keys...]");
    let want_kind = std::env::args().nth(2).unwrap_or_else(|| "fighter_kind_pikachu".into());
    let keys: Vec<String> = std::env::args().skip(3).collect();
    let lm = labels();
    let want = hash40(&want_kind);
    let root: ParamStruct = prc::open(&path).expect("failed to parse prc");
    let table_hash = hash40("fighter_param_table");
    let kind_hash = hash40("fighter_kind");

    // A FLAT PRC HAS NO FIGHTER TABLE, AND `common.prc` IS THE ONE THIS PROJECT NEEDS MOST.
    //
    // `WorkModule::get_param_float(boma, hash40("common"), hash40("turn_stick_x"))` -- the shape
    // every carry and walk decision in DK's status tree is written in -- does NOT read the
    // fighter's own params. `turn_stick_x`, `walk_stick_x`, `turn_frame`, `pass_stick_y`,
    // `pass_flick_y`, `jump_stick_y`, `jump_flick_y`, `jump_neutral_y` and
    // `throw_invincible_frame` are all absent from `fighter_param.prc`'s per-fighter entries and
    // all present in `fighter/common/param/common.prc`, which is game-wide: one value, shared by
    // every fighter. That is what makes those reads portable at all -- Pikachu gets the same
    // number DK does, by construction, with nothing to provide and nothing to copy.
    //
    // Checked by byte-searching each 40-bit hash as a little-endian u64 in both files before this
    // mode existed; this mode is so the VALUES can be read too rather than just their presence.
    // RECURSIVE, because the keys are not at the root. `common.prc`'s root holds a handful of
    // sub-structs and every one of these nine sits inside one of them, so a one-level scan finds
    // nothing and reports it as absent -- which is the wrong answer twice over.
    if !root.0.iter().any(|(h, _)| *h == table_hash) {
        println!("# {} -- flat prc, no fighter table", path);
        walk(&root, "", &lm, &keys);
        return;
    }

    for (h, v) in root.0.iter() {
        if *h != table_hash { continue; }
        if let ParamKind::List(list) = v {
            for (i, entry) in list.0.iter().enumerate() {
                let ParamKind::Struct(s) = entry else { continue };
                let is_match = s.0.iter().any(|(fh, fv)| {
                    *fh == kind_hash && matches!(fv, ParamKind::Hash(hv) if *hv == want)
                });
                if !is_match { continue; }
                println!("# {} -> index {}", want_kind, i);
                for (fh, fv) in s.0.iter() {
                    let name = lm.get(fh).cloned().unwrap_or_else(|| format!("{:#012x}", fh.0));
                    if !keys.is_empty() && !keys.iter().any(|k| k == &name) { continue; }
                    let val = match fv {
                        ParamKind::Float(f) => format!("{}", f),
                        ParamKind::I32(n) => format!("{}", n),
                        ParamKind::U32(n) => format!("{}", n),
                        ParamKind::Bool(b) => format!("{}", b),
                        ParamKind::Hash(h2) => lm.get(h2).cloned().unwrap_or_else(|| format!("{:#012x}", h2.0)),
                        other => format!("{:?}", other),
                    };
                    println!("{:<28} {}", name, val);
                }
            }
        }
    }
}

/// Print every scalar under `s`, prefixing nested names with their parent's, and filtering by
/// `keys` on the LEAF name only -- the caller knows `turn_stick_x`, not which sub-struct the game
/// files it under.
fn walk(
    s: &ParamStruct,
    prefix: &str,
    lm: &std::collections::HashMap<Hash40, String>,
    keys: &[String],
) {
    for (fh, fv) in s.0.iter() {
        let name = lm.get(fh).cloned().unwrap_or_else(|| format!("{:#012x}", fh.0));
        match fv {
            ParamKind::Struct(inner) => {
                walk(inner, &format!("{prefix}{name}/"), lm, keys);
            }
            ParamKind::List(list) => {
                for (i, e) in list.0.iter().enumerate() {
                    if let ParamKind::Struct(inner) = e {
                        walk(inner, &format!("{prefix}{name}[{i}]/"), lm, keys);
                    }
                }
            }
            other => {
                if !keys.is_empty() && !keys.iter().any(|k| k == &name) {
                    continue;
                }
                let val = match other {
                    ParamKind::Float(f) => format!("{f}"),
                    ParamKind::I32(n) => format!("{n}"),
                    ParamKind::U32(n) => format!("{n}"),
                    ParamKind::Bool(b) => format!("{b}"),
                    ParamKind::Hash(h2) => {
                        lm.get(h2).cloned().unwrap_or_else(|| format!("{:#012x}", h2.0))
                    }
                    o => format!("{o:?}"),
                };
                println!("{:<44} {}", format!("{prefix}{name}"), val);
            }
        }
    }
}
