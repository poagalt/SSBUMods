// Scales `top`-anchored hitbox offsets by the source fighter's scale ratio.
//
// tools/anim-dropin.sh corrects for Pikachu's 0.96 `scale` param by scaling the animation's `Rot`
// node, which inherits down every bone -- so a bone-anchored hitbox and its bone-local offset are
// both exact (measured: 0.00 world units on jab, f-smash, n-air and u-air).
//
// A `top`-anchored hitbox never touches the skeleton: `top` is the fighter's origin, so the offset
// is used as-is and then multiplied by the fighter's scale. That path bypasses the animation
// entirely, leaving the ratio uncorrected -- the residual 0.88-1.78 world units that remained on
// dash attack, f-tilt, u-tilt, u-smash and f-air after the animation fix. Correcting it here is
// exact arithmetic, not an approximation: reference = off * source_scale, ours = off * 0.96, so
// off * (source_scale / 0.96) makes the two equal.
//
// Hitbox SIZE is deliberately left alone. The radius is scaled by the fighter's `scale` param, so
// ours is 4-20% small, but whether the engine also folds in the anchor bone's inherited scale is
// not answerable from the animation data -- and if it does, scaling here would double-count.
// Tracked in the backlog to settle on hardware.
//
// usage: topscale <plugin-src-dir>
use std::collections::HashMap;

const PIKACHU_SCALE: f32 = 0.96;

// generated fn -> source `scale` param. Must match tools/anim-dropin.sh.
const MAP: &[(&str, f32)] = &[
    ("pikachu_game_attack11", 0.96),
    ("pikachu_game_attackdash", 1.112),
    ("pikachu_game_attacks3", 1.112),
    ("pikachu_game_attackhi3", 1.05),
    ("pikachu_game_attacks4", 1.11),
    ("pikachu_game_attacks4hi", 1.11),
    ("pikachu_game_attacks4lw", 1.11),
    ("pikachu_game_attackhi4", 1.08),
    ("pikachu_game_attackairn", 0.95),
    ("pikachu_game_attackairf", 1.007),
    ("pikachu_game_attackairb", 1.03),
    ("pikachu_game_attackairhi", 0.99),
    ("pikachu_game_attackairlw", 1.15),
];

// Splits on top-level commas only, so Hash40::new("x") and Some(1.0) stay intact.
fn spans(args: &str) -> Vec<(usize, usize)> {
    let mut out = vec![];
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, c) in args.char_indices() {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth < 0 {
                out.push((start, i));
                return out;
            }
        } else if c == ',' && depth == 0 {
            out.push((start, i));
            start = i + 1;
        }
    }
    out.push((start, args.len()));
    out
}

fn fmt(v: f32) -> String {
    if (v - v.round()).abs() < 1e-6 { format!("{:.1}", v) } else { format!("{}", v) }
}

fn main() {
    let src_dir = std::env::args().nth(1).expect("plugin src dir");
    let map: HashMap<&str, f32> = MAP.iter().copied().collect();
    let mut total = 0usize;
    let mut skipped_bone = 0usize;

    for entry in std::fs::read_dir(&src_dir).unwrap().flatten() {
        let dir = entry.path();
        if !dir.is_dir() { continue }
        let file = dir.join("mod.rs");
        if !file.exists() { continue }
        let text = std::fs::read_to_string(&file).unwrap();
        let mut out = String::new();
        let mut cur_fn = String::new();
        let mut changed = 0usize;

        for line in text.lines() {
            if let Some(p) = line.find("unsafe extern \"C\" fn ") {
                cur_fn = line[p + 21..].split('(').next().unwrap_or("").to_string();
            }
            let Some(&sscale) = map.get(cur_fn.as_str()) else { out.push_str(line); out.push('\n'); continue };
            let ratio = sscale / PIKACHU_SCALE;
            if (ratio - 1.0).abs() < 5e-4 { out.push_str(line); out.push('\n'); continue }
            let Some(ap) = line.find("macros::ATTACK(") else { out.push_str(line); out.push('\n'); continue };

            let args_at = ap + 15;
            let args = &line[args_at..];
            let sp = spans(args);
            if sp.len() < 16 { out.push_str(line); out.push('\n'); continue }
            let arg = |n: usize| -> &str { args[sp[n].0..sp[n].1].trim() };

            if arg(3) != "Hash40::new(\"top\")" {
                // Bone-anchored: already exact via the animation's inherited Rot scale.
                skipped_bone += 1;
                out.push_str(line); out.push('\n'); continue;
            }

            // Rebuild the argument list, scaling x,y,z (10..12) and the stretch endpoints (13..15).
            let mut rebuilt: Vec<String> = Vec::with_capacity(sp.len());
            for (n, _) in sp.iter().enumerate() {
                let a = arg(n);
                let scale_this = (10..=15).contains(&n);
                if !scale_this { rebuilt.push(a.to_string()); continue }
                if let Some(inner) = a.strip_prefix("Some(").and_then(|s| s.strip_suffix(")")) {
                    match inner.trim().parse::<f32>() {
                        Ok(v) => rebuilt.push(format!("Some({})", fmt(v * ratio))),
                        Err(_) => rebuilt.push(a.to_string()),
                    }
                } else {
                    match a.parse::<f32>() {
                        Ok(v) => rebuilt.push(fmt(v * ratio)),
                        Err(_) => rebuilt.push(a.to_string()),
                    }
                }
            }
            let tail_start = args_at + sp[sp.len() - 1].1;
            let newline = format!("{}{}{}", &line[..args_at], rebuilt.join(", "), &line[tail_start..]);
            out.push_str(&newline);
            out.push('\n');
            changed += 1;
        }
        if changed > 0 {
            std::fs::write(&file, out).unwrap();
            println!("  {:<10} {changed} top-anchored hitboxes scaled", dir.file_name().unwrap().to_string_lossy());
            total += changed;
        }
    }
    println!("topscale: {total} scaled, {skipped_bone} bone-anchored left alone (already exact)");
}
