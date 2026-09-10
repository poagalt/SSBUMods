// Re-anchors limb-anchored hitboxes to `top` at the world position they occupy during the
// SOURCE fighter's own animation, preserving absolute reach.
//
// Why: a hitbox offset is bone-local. On the source fighter the animation carries that bone into
// position; Pikachu plays HIS animation, so the bone stays elsewhere and the hitbox lands in the
// wrong place. Measured worst case: Falco's u-tilt second hit rides `arml`, which Falco swings to
// y=19.15 and Pikachu leaves at y=0.88.
//
// `top` is the fighter's origin rather than a skeleton bone, so a top-anchored offset is simply
// the position relative to the fighter -- which is what bone_world() returns.
//
// Offsets and sizes are scaled by source_scale/pikachu_scale, because the game multiplies hitbox
// coordinates by the fighter's `scale` param and those differ per fighter.
//
// Little Mac's f-smash is deliberately NOT handled here. Its missing range is root motion (a
// 21.45-unit lunge), not bone placement, and it is fixed by reproducing the lunge instead.
//
// usage: reanchor <plugin-src-dir> <fk-json-dir>
use reach::*;
use std::collections::HashMap;

const PIKACHU_SCALE: f32 = 0.96;

// generated fn -> (source fighter, animation stem, source scale)
const MAP: &[(&str, &str, &str, f32)] = &[
    ("pikachu_game_attack11",    "chrom",     "c00attack11",    0.96),
    ("pikachu_game_attackdash",  "elight",    "c00attackdash",  1.112),
    ("pikachu_game_attacks3",    "elight",    "c01attacks3s",   1.112),
    ("pikachu_game_attackhi3",   "falco",     "c02attackhi3",   1.05),
    ("pikachu_game_attackhi4",   "fox",       "c04attackhi4",   1.08),
    ("pikachu_game_attackairn",  "palutena",  "c05attackairn",  0.95),
    ("pikachu_game_attackairf",  "demon",     "c05attackairf",  1.007),
    ("pikachu_game_attackairhi", "kamui",     "c05attackairhi", 0.99),
];

// Only genuine limbs. `haver`/`havel` are the ITEM-HOLD bones: for a ported move they stand in
// for a weapon Pikachu does not have, and absolute reach is the wrong model there. Palutena's
// n-air proved it -- her staff hitboxes ring HER body, so transplanting them absolutely put them
// in a shell above a much smaller Pikachu and the move lost its range. Falco's u-tilt, anchored
// to a real limb, improved by the same change. Weapon-anchored moves are fixed by the appendage
// work instead.
const LIMBS: &[&str] = &[
    "armr", "arml", "kneer", "kneel", "toel", "toer",
    "shoulderl", "shoulderr", "handr", "handl",
];

fn nth_arg(args: &str, n: usize) -> Option<&str> {
    // splits on top-level commas only, so Hash40::new("x") and Some(1.0) stay intact
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut idx = 0usize;
    for (i, c) in args.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                if idx == n { return Some(args[start..i].trim()); }
                idx += 1;
                start = i + 1;
            }
            _ => {}
        }
    }
    if idx == n { return Some(args[start..].trim()); }
    None
}

fn main() {
    let src_dir = std::env::args().nth(1).expect("plugin src dir");
    let fk_dir = std::env::args().nth(2).expect("fk json dir");

    let mut cache: HashMap<String, (Vec<String>, Vec<i64>, Vec<M4>, HashMap<String, Vec<M4>>)> =
        Default::default();
    let mut changed_total = 0;

    for entry in std::fs::read_dir(&src_dir).unwrap().flatten() {
        let dir = entry.path();
        if !dir.is_dir() { continue; }
        let file = dir.join("mod.rs");
        if !file.exists() { continue; }
        let text = std::fs::read_to_string(&file).unwrap();
        let mut out = String::new();
        let mut cur_fn = String::new();
        let mut frame = 0f32;
        let mut changed = 0;

        for line in text.lines() {
            if let Some(p) = line.find("unsafe extern \"C\" fn ") {
                cur_fn = line[p + 21..].split('(').next().unwrap_or("").to_string();
                frame = 0.0;
            }
            if let Some(p) = line.find("frame(agent.lua_state_agent, ") {
                if let Some(v) = line[p + 29..].split(')').next().and_then(|s| s.trim().parse::<f32>().ok()) {
                    frame = v;
                }
            } else if let Some(p) = line.find("wait(agent.lua_state_agent, ") {
                if let Some(v) = line[p + 28..].split(')').next().and_then(|s| s.trim().parse::<f32>().ok()) {
                    frame += v;
                }
            }

            let Some(spec) = MAP.iter().find(|m| m.0 == cur_fn) else { out.push_str(line); out.push('\n'); continue };
            let Some(ap) = line.find("macros::ATTACK(") else { out.push_str(line); out.push('\n'); continue };

            let args = &line[ap + 15..];
            let bone_arg = nth_arg(args, 3).unwrap_or("");
            let bone = bone_arg.trim_start_matches("Hash40::new(\"").trim_end_matches("\")").to_string();
            if !LIMBS.contains(&bone.as_str()) { out.push_str(line); out.push('\n'); continue; }

            let (_, fighter, anim, sscale) = *spec;
            let key = format!("{fighter}/{anim}");
            let e = cache.entry(key).or_insert_with(|| {
                let (n, p, b) = load_skel(&format!("{fk_dir}/{fighter}_skel.json"));
                let a = load_anim(&format!("{fk_dir}/{fighter}_{anim}.json"));
                (n, p, b, a)
            });

            let Some(bw) = bone_world(&e.0, &e.1, &e.2, &e.3, &bone, frame.max(0.0) as usize) else {
                eprintln!("  ! {cur_fn}: bone {bone} not in {fighter}'s skeleton, left alone");
                out.push_str(line); out.push('\n'); continue;
            };

            let size: f32 = nth_arg(args, 9).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let ox: f32 = nth_arg(args, 10).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let oy: f32 = nth_arg(args, 11).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let oz: f32 = nth_arg(args, 12).and_then(|s| s.parse().ok()).unwrap_or(0.0);

            let ratio = sscale / PIKACHU_SCALE;
            let w = transform_point([ox, oy, oz], bw);
            let (nx, ny, nz) = (w[0] * ratio, w[1] * ratio, w[2] * ratio);
            let nsize = size * ratio;

            let mut newline = line.to_string();
            newline = newline.replace(&format!("Hash40::new(\"{bone}\")"), "Hash40::new(\"top\")");
            // replace the size,x,y,z run in place
            let old_run = format!("{}, {}, {}, {}", fmt(size), fmt(ox), fmt(oy), fmt(oz));
            let new_run = format!("{}, {}, {}, {}", fmt(nsize), fmt(nx), fmt(ny), fmt(nz));
            if newline.contains(&old_run) {
                newline = newline.replacen(&old_run, &new_run, 1);
                changed += 1;
                println!(
                    "  {cur_fn:<30} f{:<4} {bone:<10} ({:.1},{:.1},{:.1}) -> top ({:.2},{:.2},{:.2}) size {:.1}->{:.2}",
                    frame, ox, oy, oz, nx, ny, nz, size, nsize
                );
            } else {
                eprintln!("  ! {cur_fn}: could not locate '{old_run}', left alone");
            }
            out.push_str(&newline);
            out.push('\n');
        }
        if changed > 0 {
            std::fs::write(&file, out).unwrap();
            println!("{} : {changed} hitboxes re-anchored", dir.file_name().unwrap().to_string_lossy());
            changed_total += changed;
        }
    }
    println!("\ntotal re-anchored: {changed_total}");
}

fn fmt(v: f32) -> String {
    if (v - v.round()).abs() < 1e-6 { format!("{:.1}", v) } else { format!("{}", v) }
}
