// Traces every hitbox of a ported move frame by frame, twice:
//
//   REFERENCE -- the source fighter's own skeleton and animation, times the source's `scale`
//                param. This is the ground truth: exactly where that character's hitbox is.
//   OURS      -- Pikachu's skeleton driven by the SAME animation (ADR 0003 ships it), times
//                Pikachu's 0.96 scale. Bones the animation names but Pikachu lacks simply drop
//                out, and bones Pikachu has but the animation omits stay at his bind pose --
//                which is precisely what the game does with a dropped-in animation.
//
// The two differ only by whatever the drop-in cannot carry across, so the deviation this prints
// is the mod's real hitbox error, per frame, in world units. That turns "does it look right"
// into a number, and feeds the side-by-side render.
//
// usage: hbtrace <acmd.txt> <game_fn> <src_fighter> <anim_stem> <src_scale> [--json]
use reach::*;
use std::collections::HashMap;

const PIKACHU_SCALE: f32 = 0.96;

#[derive(Clone, Debug)]
struct Hitbox {
    id: u32,
    bone: String,
    size: f32,
    off: [f32; 3],
}

// Splits on top-level commas only, so Hash40::new("x") and Some(1.0) stay intact.
fn nth(args: &str, n: usize) -> Option<&str> {
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut idx = 0usize;
    for (i, c) in args.char_indices() {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
        } else if c == ',' && depth == 0 {
            if idx == n {
                return Some(args[start..i].trim());
            }
            idx += 1;
            start = i + 1;
        }
    }
    if idx == n {
        return Some(args[start..].trim());
    }
    None
}

fn fnum(args: &str, n: usize) -> f32 {
    nth(args, n).and_then(|s| s.parse().ok()).unwrap_or(0.0)
}

fn num_after(line: &str, key: &str) -> Option<f32> {
    let p = line.find(key)? + key.len();
    let rest = &line[p..];
    let end = rest.find(')')?;
    rest[..end].trim().parse().ok()
}

struct Sample {
    gf: f32,
    id: u32,
    bone: String,
    size: f32,
    reference: [f32; 3],
    ours: [f32; 3],
    dev: f32,
    ok: bool,
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 5 {
        eprintln!("usage: hbtrace <acmd.txt> <game_fn> <fighter> <anim_stem> <scale> [--json]");
        std::process::exit(2);
    }
    let (path, want, fighter, stem) = (&a[0], &a[1], &a[2], &a[3]);
    let sscale: f32 = a[4].parse().unwrap();
    let as_json = a.iter().any(|x| x == "--json");

    let (sn, sp, sb) = load_skel(&format!("work/fk/{fighter}_skel.json"));
    // The SHIPPED skeleton, not the vanilla one: tools/add-bones.sh grafts on Sword1 and Stick,
    // and measuring against vanilla would report those hitboxes as unmeasurable.
    let (pn, pp, pb) = load_skel("work/fk/pikachu_shipped_skel.json");
    let anim = load_anim(&format!("work/fk/{fighter}_{stem}.json"));

    // Walk the ACMD, keeping a live hitbox set and the animation frame each game frame maps to.
    let text = std::fs::read_to_string(path).unwrap();
    let mut infn = false;
    let mut frame = 1.0f32;
    let mut rate = 1.0f32; // FT_MOTION_RATE: animation frames advanced per game frame
    let mut anim_at = 0.0f32;
    let mut live: HashMap<u32, Hitbox> = HashMap::new();
    let mut timeline: Vec<(f32, f32, Vec<Hitbox>)> = vec![];

    for line in text.lines() {
        if line.contains("unsafe extern \"C\" fn ") {
            infn = line.contains(&format!("fn {want}("));
        }
        if !infn {
            continue;
        }

        // A frame()/wait() boundary is where the current hitbox set persisted until, so the
        // state is recorded for each whole frame crossed rather than only at event boundaries.
        let target = if line.contains("frame(agent.lua_state_agent, ") {
            num_after(line, "frame(agent.lua_state_agent, ")
        } else if line.contains("wait(agent.lua_state_agent, ") {
            num_after(line, "wait(agent.lua_state_agent, ").map(|v| frame + v)
        } else {
            None
        };
        if let Some(to) = target {
            while frame < to {
                if !live.is_empty() {
                    let mut v: Vec<Hitbox> = live.values().cloned().collect();
                    v.sort_by_key(|h| h.id);
                    timeline.push((frame, anim_at, v));
                }
                frame += 1.0;
                anim_at += rate;
            }
            frame = to;
        }

        if let Some(v) = num_after(line, "FT_MOTION_RATE(agent, ") {
            rate = v;
        }
        if line.contains("AttackModule::clear_all") {
            live.clear();
        }
        if let Some(p) = line.find("macros::ATTACK(") {
            let args = &line[p + 15..];
            let id: u32 = nth(args, 1).and_then(|s| s.parse().ok()).unwrap_or(0);
            let bone = nth(args, 3)
                .unwrap_or("")
                .trim_start_matches("Hash40::new(\"")
                .trim_end_matches("\")")
                .to_string();
            live.insert(
                id,
                Hitbox {
                    id,
                    bone,
                    size: fnum(args, 9),
                    off: [fnum(args, 10), fnum(args, 11), fnum(args, 12)],
                },
            );
        }
    }

    // Evaluate both skeletons on every active frame.
    let mut out: Vec<Sample> = vec![];
    for (gf, af, hbs) in &timeline {
        for h in hbs {
            let ai = af.max(0.0) as usize;
            // `top` is the fighter's origin rather than a skeleton bone, so its transform is the
            // identity on both fighters. Such a hitbox still deviates by the scale ratio, so it
            // is measured rather than skipped as an absent bone.
            let (sw, ow) = if h.bone == "top" {
                (Some(ID), Some(ID))
            } else {
                (bone_world(&sn, &sp, &sb, &anim, &h.bone, ai),
                 bone_world(&pn, &pp, &pb, &anim, &h.bone, ai))
            };
            match (sw, ow) {
                (Some(sw), Some(ow)) => {
                    let mut r = transform_point(h.off, sw);
                    let mut o = transform_point(h.off, ow);
                    for i in 0..3 {
                        r[i] *= sscale;
                        o[i] *= PIKACHU_SCALE;
                    }
                    let dev = ((r[0] - o[0]).powi(2) + (r[1] - o[1]).powi(2) + (r[2] - o[2]).powi(2)).sqrt();
                    out.push(Sample { gf: *gf, id: h.id, bone: h.bone.clone(), size: h.size * sscale,
                                      reference: r, ours: o, dev, ok: true });
                }
                _ => out.push(Sample { gf: *gf, id: h.id, bone: h.bone.clone(), size: h.size * sscale,
                                       reference: [0.0; 3], ours: [0.0; 3], dev: f32::NAN, ok: false }),
            }
        }
    }

    if as_json {
        let hits: Vec<String> = out.iter().map(|s| format!(
            "{{\"f\":{},\"id\":{},\"bone\":\"{}\",\"size\":{:.3},\"ok\":{},\
             \"ref\":[{:.3},{:.3},{:.3}],\"ours\":[{:.3},{:.3},{:.3}],\"dev\":{:.3}}}",
            s.gf, s.id, s.bone, s.size, s.ok,
            s.reference[0], s.reference[1], s.reference[2], s.ours[0], s.ours[1], s.ours[2],
            if s.ok { s.dev } else { -1.0 })).collect();
        println!("{{\"move\":\"{want}\",\"fighter\":\"{fighter}\",\"scale\":{sscale},\"hits\":[{}]}}", hits.join(","));
    } else {
        println!("{want} ({fighter}, scale {sscale}) -- {} active hitbox-frames", out.len());
        println!("  {:>4} {:>3} {:<9} {:>21} {:>21} {:>7}", "f", "id", "bone", "REFERENCE x/y/z", "OURS x/y/z", "dev");
        for s in &out {
            if !s.ok {
                println!("  {:>4} {:>3} {:<9}  bone absent from one skeleton", s.gf, s.id, s.bone);
                continue;
            }
            println!("  {:>4} {:>3} {:<9} {:>7.2}{:>7.2}{:>7.2} {:>7.2}{:>7.2}{:>7.2} {:>7.2}",
                     s.gf, s.id, s.bone, s.reference[0], s.reference[1], s.reference[2],
                     s.ours[0], s.ours[1], s.ours[2], s.dev);
        }
        // Report the sample count alongside the mean. A hitbox on a bone Pikachu lacks is
        // skipped, and averaging over an empty set printed "mean deviation 0.00" -- which reads
        // as a perfect result when it actually means nothing was measured at all. Say so loudly.
        let v: Vec<&Sample> = out.iter().filter(|s| s.ok).collect();
        let skipped = out.len() - v.len();
        if v.is_empty() {
            let bones: Vec<&str> = out.iter().map(|s| s.bone.as_str()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
            println!("\n  NOT MEASURED: all {} samples skipped -- bone(s) {} absent from Pikachu",
                     out.len(), bones.join(", "));
            std::process::exit(3);
        }
        let worst = v.iter().fold((0.0f32, String::new()), |acc, s| {
            if s.dev > acc.0 { (s.dev, format!("f{} id{} {}", s.gf, s.id, s.bone)) } else { acc }
        });
        let mean = v.iter().map(|s| s.dev).sum::<f32>() / v.len() as f32;
        print!("\n  mean deviation {:.2}, worst {:.2} world units at {} (n={})",
               mean, worst.0, worst.1, v.len());
        if skipped > 0 {
            let bones: Vec<&str> = out.iter().filter(|s| !s.ok).map(|s| s.bone.as_str())
                .collect::<std::collections::BTreeSet<_>>().into_iter().collect();
            print!("  -- {} SKIPPED on absent bone(s) {}", skipped, bones.join(", "));
        }
        println!();
    }
}
