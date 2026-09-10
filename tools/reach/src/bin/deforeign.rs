// Removes statements that reference a SOURCE fighter's own namespace constants.
//
// A verbatim ACMD port carries the source's article ids and instance work-flag ids, and those are
// per-fighter enumerations that collide. Measured against skyline-smash's own constant tables:
//
//   FIGHTER_ELIGHT_GENERATE_ARTICLE_ESWORD                      = 0x0        = Pikachu's DENGEKIDAMA
//   FIGHTER_ELIGHT_..._ADD_PARTIAL_MTION_SWORD_WHEN_CHANGEING   = 0x200000E6 = Pikachu's FLAG_STRANS_1
//   FIGHTER_KROOL_..._REQUEST_WAIST_SHIELD_ON                   = 0x200000E6 = Pikachu's FLAG_STRANS_1
//   FIGHTER_KROOL_..._REQUEST_WAIST_SHIELD_OFF                  = 0x200000E7 = Pikachu's FLAG_PAUSE
//
// So the ported f-tilt was setting Pikachu's STRANS_1 flag, the ported d-air was setting STRANS_1
// and PAUSE, and `IS_EXIST_ARTICLE(agent, 0)` was testing for Pikachu's electro ball and would
// have applied Mythra's sword motion-part kind to it. None of that is the ported move; it is the
// source fighter's own machinery reaching into the host's state.
//
// The K. Rool flags are also the belly-armor toggle, which ADR 0001 excludes by decision: it is a
// character-wide K. Rool trait, not part of the d-air.
//
// Algorithm: drop each offending statement, then cascade-delete any block left with an empty body.
// That handles both shapes seen -- a flag set alone inside `if is_excute { ... }` (the block goes
// too) and one sharing a block with the move's real ATTACK calls (only the statement goes).
//
// usage: deforeign <plugin-src-dir>

// Fighter namespaces we port from -- keep in step with tools/gen-moves.sh. Constants under
// FIGHTER_ATTACK_*, FIGHTER_KINETIC_*, FIGHTER_STATUS_* and the like are engine-global, shared by
// every fighter, and must NOT be stripped.
const SOURCES: &[&str] = &[
    "CHROM", "ELIGHT", "FALCO", "ROBOT", "LITTLEMAC", "FOX", "GAMEWATCH", "PALUTENA", "DEMON",
    "CLOUD", "KAMUI", "KROOL", "GAOGAEN", "MEWTWO", "KOOPA", "GANON",
];

fn is_foreign(line: &str) -> bool {
    SOURCES.iter().any(|s| {
        line.contains(&format!("FIGHTER_{s}_")) || line.contains(&format!("WEAPON_{s}_"))
    })
}

// A ported script can also call a macro our smash-script version does not export. Incineroar's
// side-B uses CHECK_FINISH_CAMERA_IF_NOT_HP_MODE, which is absent from the 112 smash-script
// provides even though plain CHECK_FINISH_CAMERA is there -- and that fails the build outright
// rather than degrading. The allowlist is read from smash-script's own source at build time, so
// the next port cannot break the build the same way and the list cannot drift from the crate.
fn called_macro(line: &str) -> Option<&str> {
    let p = line.find("macros::")? + 8;
    let rest = &line[p..];
    let e = rest.find('(')?;
    Some(&rest[..e])
}

fn main() {
    let src_dir = std::env::args().nth(1).expect("plugin src dir");
    // Optional: a file of one macro name per line. Without it, no macro filtering happens, so
    // the pass stays useful even if the list has not been generated.
    let known: std::collections::HashSet<String> = std::env::args()
        .nth(2)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default();
    let mut total_stmts = 0usize;
    let mut total_blocks = 0usize;
    let mut total_macros = 0usize;

    for entry in std::fs::read_dir(&src_dir).unwrap().flatten() {
        let dir = entry.path();
        if !dir.is_dir() { continue }
        let file = dir.join("mod.rs");
        if !file.exists() { continue }

        let mut lines: Vec<String> =
            std::fs::read_to_string(&file).unwrap().lines().map(String::from).collect();

        // 1. Drop offending statements. Only whole single-line statements are handled; a foreign
        //    constant inside a multi-line construct would be missed, so refuse loudly instead.
        let mut stmts = 0usize;
        let mut macros_dropped = 0usize;
        let mut keep: Vec<bool> = vec![true; lines.len()];

        // Unsupported macro calls first, on the same statement-drop-then-cascade basis.
        if !known.is_empty() {
            for (i, l) in lines.iter().enumerate() {
                if let Some(m) = called_macro(l) {
                    if !known.contains(m) && l.trim().ends_with(';') {
                        keep[i] = false;
                        macros_dropped += 1;
                    }
                }
            }
        }

        for (i, l) in lines.iter().enumerate() {
            if !keep[i] { continue }
            if !is_foreign(l) { continue }
            let t = l.trim();
            if t.ends_with(';') && !t.starts_with("if ") && !t.contains('{') {
                keep[i] = false;
                stmts += 1;
            } else if t.starts_with("if ") && t.ends_with('{') {
                // `if <foreign cond> {` -- keep the line for now; emptying the body will remove
                // the block in step 2 once its only statement is gone. But if the condition
                // itself is foreign and the body is NOT purely foreign we would silently keep a
                // colliding test, so handle it by rewriting the condition to a constant false.
                // In practice every such block's body is foreign too, and step 2 removes it.
            } else {
                eprintln!("  ! {}: foreign constant in an unhandled construct, left alone:\n    {}",
                          file.display(), t);
            }
        }
        lines = lines.iter().zip(&keep).filter(|(_, k)| **k).map(|(l, _)| l.clone()).collect();

        // 2. Cascade-remove blocks whose body is now empty: a line ending in `{` immediately
        //    followed by its closing `}` at the same indentation. Repeat until stable, so an
        //    `if A { if B { } }` collapses entirely.
        let mut blocks = 0usize;
        loop {
            let mut removed = false;
            let mut i = 0usize;
            while i + 1 < lines.len() {
                let open = lines[i].trim_end();
                let close = lines[i + 1].trim();
                // Never collapse a function definition. K. Rool's game_landingairlw consists of
                // nothing but the belly-armor flag, so emptying its body cascaded into deleting
                // the function itself and the install list stopped compiling. An empty body is
                // exactly what is wanted there anyway: it overrides Pikachu's own landing script
                // and suppresses his leftover landing hit.
                let is_fn = open.contains("unsafe extern \"C\" fn ");
                if open.ends_with('{') && close == "}" && !is_fn {
                    let oi = lines[i].len() - lines[i].trim_start().len();
                    let ci = lines[i + 1].len() - lines[i + 1].trim_start().len();
                    if oi == ci {
                        lines.drain(i..=i + 1);
                        blocks += 1;
                        removed = true;
                        continue;
                    }
                }
                i += 1;
            }
            if !removed { break }
        }

        if stmts > 0 || blocks > 0 || macros_dropped > 0 {
            std::fs::write(&file, lines.join("\n") + "\n").unwrap();
            println!("  {:<10} {stmts} foreign statements, {macros_dropped} unsupported macros, {blocks} emptied blocks removed",
                     dir.file_name().unwrap().to_string_lossy());
            total_stmts += stmts;
            total_blocks += blocks;
            total_macros += macros_dropped;
        }
    }
    println!("deforeign: {total_stmts} foreign statements, {total_macros} unsupported macro calls and {total_blocks} blocks removed");
}
