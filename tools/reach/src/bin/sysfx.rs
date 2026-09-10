// Strips, from the ported effect scripts, every effect the source fighter owns but Pikachu
// cannot load -- leaving only the engine-global `sys_*` ones.
//
// ADR 0002: the mod ships no effect archives, so it works without CSK. A source's own effects
// (`littlemac_attack_line`, `fox_smash_arc`, `chrom_sword`) live in that fighter's archive, which
// is not loaded for Pikachu. The generic `sys_*` set is always resident, so those carry across.
//
// TWO ORDERING TRAPS, both handled:
//
// 1. `LAST_EFFECT_SET_COLOR`, `LAST_EFFECT_SET_RATE`, `LAST_EFFECT_SET_ALPHA`,
//    `LAST_EFFECT_SET_OFFSET_TO_CAMERA_FLAT` and `EffectModule::enable_sync_init_pos_last` all
//    act on whichever effect was spawned LAST. Dropping a spawn while keeping its modifiers
//    would silently retarget them at the previous, surviving effect -- recolouring a smash flash
//    with a sword trail's colour. A dropped spawn therefore takes its trailing modifiers with it.
//
// 2. `AFTER_IMAGE*` draws a trail from `tex_<fighter>_*` textures in the source's model. Those
//    are not shipped, so every AFTER_IMAGE statement goes regardless of its arguments.
//
// usage: sysfx <plugin-src-dir>

// The classification is STRUCTURAL, not a hand-kept list of macro names. An earlier version
// enumerated the spawn macros and missed `EFFECT_FOLLOW_FLIP`, which let `koopa_scratch` through
// -- and the SSBU effect macros have enough near-identical variants
// (EFFECT_FOLLOW, _ALPHA, _WORK, _NO_STOP, _FLIP, _FLIP_ALPHA, ...) that any hand list will rot.
// So: anything whose macro name contains EFFECT and is not a known last-effect modifier is
// treated as a spawn and judged by its first Hash40 argument.

// Act on whichever effect was spawned last, so they follow whatever they were modifying.
const MODIFIER: &[&str] = &[
    "LAST_EFFECT_SET_", "enable_sync_init_pos_last",
];

fn macro_name(line: &str) -> Option<&str> {
    let p = line.find("macros::")? + 8;
    let rest = &line[p..];
    let e = rest.find('(')?;
    Some(&rest[..e])
}

fn first_hash(line: &str) -> Option<&str> {
    let p = line.find("Hash40::new(\"")? + 13;
    let rest = &line[p..];
    let e = rest.find('"')?;
    Some(&rest[..e])
}

fn is_modifier(line: &str) -> bool {
    MODIFIER.iter().any(|m| line.contains(m))
}
fn is_trail(line: &str) -> bool {
    macro_name(line).map_or(false, |m| m.contains("AFTER_IMAGE"))
}
fn is_spawn(line: &str) -> bool {
    !is_modifier(line) && macro_name(line).map_or(false, |m| m.contains("EFFECT"))
}
// Source fighter namespaces, for the independent gate at the end. Same list as deforeign.rs.
const SOURCES: &[&str] = &[
    "chrom", "elight", "falco", "robot", "littlemac", "fox", "gamewatch", "palutena", "demon",
    "cloud", "kamui", "krool", "gaogaen", "mewtwo", "koopa", "ganon",
];

// An effect Pikachu can load: the engine-global sys_ set, or the null placeholder.
fn loadable(name: &str) -> bool {
    name.starts_with("sys_") || name == "null"
}

fn main() {
    let src_dir = std::env::args().nth(1).expect("plugin src dir");
    let mut dropped_spawn = 0usize;
    let mut dropped_mod = 0usize;
    let mut dropped_trail = 0usize;
    let mut kept = 0usize;
    let mut blocks = 0usize;

    for entry in std::fs::read_dir(&src_dir).unwrap().flatten() {
        let dir = entry.path();
        if !dir.is_dir() { continue }
        let file = dir.join("mod.rs");
        if !file.exists() { continue }

        let mut lines: Vec<String> =
            std::fs::read_to_string(&file).unwrap().lines().map(String::from).collect();
        let mut keep = vec![true; lines.len()];
        let mut in_effect_fn = false;
        // True while the statement we last decided on was a DROPPED spawn, so its trailing
        // modifiers must go too. Reset by the next spawn or by leaving the block.
        let mut dropping_modifiers = false;

        for i in 0..lines.len() {
            let l = &lines[i];
            if l.contains("unsafe extern \"C\" fn ") {
                in_effect_fn = l.contains("fn pikachu_effect_");
                dropping_modifiers = false;
                continue;
            }
            if !in_effect_fn { continue }
            // A block boundary or a frame advance ends the "last effect" association.
            if l.trim() == "}" || l.contains("frame(agent.lua_state_agent") || l.contains("wait(agent.lua_state_agent") {
                dropping_modifiers = false;
                continue;
            }

            if is_trail(l) {
                keep[i] = false; dropped_trail += 1; dropping_modifiers = true; continue;
            }
            if is_spawn(l) {
                match first_hash(l) {
                    Some(name) if loadable(name) => { kept += 1; dropping_modifiers = false; }
                    _ => { keep[i] = false; dropped_spawn += 1; dropping_modifiers = true; }
                }
                continue;
            }
            if is_modifier(l) && dropping_modifiers {
                keep[i] = false; dropped_mod += 1;
            }
        }

        let before = lines.len();
        lines = lines.iter().zip(&keep).filter(|(_, k)| **k).map(|(l, _)| l.clone()).collect();
        if lines.len() == before { continue }

        // Cascade-remove blocks left with an empty body, never a function definition -- an empty
        // effect function is exactly what we want when nothing survived.
        loop {
            let mut removed = false;
            let mut i = 0usize;
            while i + 1 < lines.len() {
                let open = lines[i].trim_end();
                let close = lines[i + 1].trim();
                let is_fn = open.contains("unsafe extern \"C\" fn ");
                if open.ends_with('{') && close == "}" && !is_fn {
                    let oi = lines[i].len() - lines[i].trim_start().len();
                    let ci = lines[i + 1].len() - lines[i + 1].trim_start().len();
                    if oi == ci { lines.drain(i..=i + 1); blocks += 1; removed = true; continue }
                }
                i += 1;
            }
            if !removed { break }
        }
        std::fs::write(&file, lines.join("\n") + "\n").unwrap();
        println!("  {:<10} filtered", dir.file_name().unwrap().to_string_lossy());
    }
    println!("sysfx: kept {kept} sys_ effects; dropped {dropped_spawn} fighter-owned effects, \
              {dropped_mod} orphaned modifiers, {dropped_trail} trails; collapsed {blocks} blocks");

    // Prove the postcondition by re-reading what was written, using an INDEPENDENT criterion.
    //
    // The first version of this gate reused is_spawn/loadable -- the filter's own predicates --
    // so it could not fail by construction: anything the filter classified as unloadable had
    // already been dropped, and anything it misclassified the gate misclassified identically. A
    // check that shares its predicate with the thing it checks is a tautology dressed as
    // verification. Injecting `littlemac_attack_arc` after the pass exited 0.
    //
    // This instead looks for any Hash40 name prefixed with a SOURCE FIGHTER's name, anywhere in
    // an effect function, regardless of which macro carries it. That catches a missed macro
    // variant, a new macro family, and a fighter-owned trail texture alike.
    let mut leaks: Vec<String> = vec![];
    for entry in std::fs::read_dir(&src_dir).unwrap().flatten() {
        let file = entry.path().join("mod.rs");
        if !file.exists() { continue }
        let who = entry.file_name().to_string_lossy().to_string();
        let mut in_effect_fn = false;
        for l in std::fs::read_to_string(&file).unwrap().lines() {
            if l.contains("unsafe extern \"C\" fn ") {
                in_effect_fn = l.contains("fn pikachu_effect_");
                continue;
            }
            if !in_effect_fn { continue }
            let mut rest = l;
            while let Some(p) = rest.find("Hash40::new(\"") {
                rest = &rest[p + 13..];
                let Some(e) = rest.find('"') else { break };
                let name = &rest[..e];
                let bare = name.strip_prefix("tex_").unwrap_or(name);
                if let Some(f) = SOURCES.iter().find(|f| bare.starts_with(&format!("{f}_"))) {
                    leaks.push(format!("{who}: {name} (belongs to {f})"));
                }
                rest = &rest[e..];
            }
        }
    }
    if !leaks.is_empty() {
        eprintln!("sysfx FAILED -- effects Pikachu cannot load survived the filter:");
        for l in &leaks { eprintln!("  {l}") }
        std::process::exit(1);
    }
    println!("sysfx: verified -- every surviving effect loads without CSK");
}
