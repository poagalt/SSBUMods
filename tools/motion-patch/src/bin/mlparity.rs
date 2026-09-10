// Compares EVERY comparable field of a ported motion entry against the source fighter's.
//
// WAS blend.rs, WHICH CHECKED ONE FIELD OUT OF FOUR. `blend_frames` governs how a move feels
// coming in, and that was worth its own tool -- but the same entry also carries `cancel_frame`
// (the FAF, so how many frames the move locks you out for), `xlu_start` / `xlu_end` (the
// intangibility window), `no_stop_intp`, and the animation flag set. tools/motion-patch patches
// all of them and NOTHING VERIFIED ANY OF THEM. FAF is frame data, and "exact frame data" is the
// whole point of the project, so having it patched-but-unchecked was the largest unmeasured claim
// left in the mod.
//
// tools/inventory.sh does print a per-move faf column, but it is a status table, not a gate, and
// it compares only the moves in its own table rather than all 77.
//
// WHAT IS DELIBERATELY NOT COMPARED, and why each is legitimately different:
//   game_script, scripts   name Pikachu's own ACMD functions -- that is the point of the port
//   animations             name the files we ship under Pikachu's paths
// Everything else must match the source exactly.
//
// PIKACHU'S OWN VALUE IS INFORMATIONAL. Requiring the motion to be on all three lists dropped
// every synthesised entry -- 31 of 77 -- as "not comparable", when the comparison that matters is
// source against ours and his number is only there to show what we changed away from. That
// excluded exactly the entries this moveset creates from scratch.
//
// usage: mlparity <extracted> <ours.bin> <fighter>:<motion>...
use hash40::hash40;
use motion_lib::mlist::{Extra, Flags, Motion};

/// Fields that must match, rendered as strings so one loop can report all of them.
fn fields(m: &Motion) -> Vec<(&'static str, String)> {
    let e = m.extra.clone().unwrap_or(Extra {
        xlu_start: 0,
        xlu_end: 0,
        cancel_frame: 0,
        no_stop_intp: false,
    });
    vec![
        ("blend", m.blend_frames.to_string()),
        ("faf", e.cancel_frame.to_string()),
        ("xlu", format!("{}-{}", e.xlu_start, e.xlu_end)),
        ("nostopintp", e.no_stop_intp.to_string()),
        ("flags", flagstr(&m.flags)),
    ]
}

// Only the flags that are set, so a diff reads as "we are missing `move`" rather than as a wall of
// falses. `Flags` has no iterator, so this is the Debug form filtered -- ugly, and better than a
// hand-maintained list of field names that would silently stop covering a new flag.
fn flagstr(f: &Flags) -> String {
    let dbg = format!("{f:?}");
    let mut set: Vec<&str> = Vec::new();
    for part in dbg.trim_start_matches("Flags").trim_matches(|c| c == '{' || c == '}' || c == ' ').split(',') {
        let mut kv = part.splitn(2, ':');
        let (Some(k), Some(v)) = (kv.next(), kv.next()) else { continue };
        if v.trim() == "true" {
            set.push(k.trim());
        }
    }
    if set.is_empty() { "-".to_string() } else { set.join("+") }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let ex = a.next().unwrap();
    let ours_path = a.next().unwrap();
    let pk = motion_lib::open(format!("{ex}/fighter/pikachu/motion/body/c00/motion_list.bin")).unwrap();
    let ours = motion_lib::open(&ours_path).unwrap();

    // REFUSE TO REPORT CLEAN ON ZERO WORK. Run with no motions this printed an empty table and
    // "0 motion(s) wrong", which is exactly how a check earns trust it has not done anything to
    // deserve. That pattern has produced six false cleans on this project. No specs is a usage
    // error, not a pass.
    let specs: Vec<String> = a.collect();
    if specs.is_empty() {
        eprintln!("no motions given -- usage: mlparity <extracted> <ours.bin> <fighter>:<motion>...");
        eprintln!("a run with nothing to check is a usage error, not a pass");
        std::process::exit(2);
    }

    println!("{:<24} {:<10} {:<12} {:>8} {:>8}", "motion", "source", "field", "source", "ours");
    let mut wrong = 0usize;
    let mut missing = 0usize;
    let mut checked = 0usize;
    for spec in specs {
        let (fighter, motion) = spec.split_once(':').unwrap();
        let h = hash40(motion);
        let sm = motion_lib::open(format!("{ex}/fighter/{fighter}/motion/body/c00/motion_list.bin")).unwrap();
        let (Some(s), Some(o)) = (sm.list.get(&h), ours.list.get(&h)) else {
            let which = if sm.list.get(&h).is_none() { "the source lacks it" } else { "we do not ship it" };
            println!("{motion:<24} {fighter:<10} -- not comparable ({which})");
            missing += 1;
            continue;
        };
        let mine = pk.list.get(&h);
        for ((name, sv), (_, ov)) in fields(s).into_iter().zip(fields(o)) {
            checked += 1;
            if sv == ov {
                continue;
            }
            wrong += 1;
            let was = mine
                .map(|m| fields(m).into_iter().find(|(n, _)| *n == name).map(|(_, v)| v).unwrap_or_default())
                .unwrap_or_else(|| "-".to_string());
            println!("{motion:<24} {fighter:<10} {name:<12} {sv:>8} {ov:>8}  <-- was {was}");
        }
    }
    println!("\n{wrong} field(s) wrong across {checked} checked, {missing} motion(s) not comparable");
    if wrong > 0 {
        std::process::exit(1);
    }
}
