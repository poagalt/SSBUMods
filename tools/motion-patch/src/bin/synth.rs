// Adds motion entries that Pikachu lacks, so ported special-move status scripts have
// something to MotionModule::change_motion into.
//
// Each new entry is cloned from an existing Pikachu entry (so `scripts` points at real Pikachu
// files that certainly load), then given the source fighter's `extra` (FAF + intangibility),
// `flags` and `game_script`.
//
// The last column additionally adopts the SOURCE's animation filename. Under the original
// script-first design every synthesised entry kept Pikachu's animation; under ADR 0003 an entry
// whose source has its own animation should name it. Little Mac's angled f-smashes forced this:
// Pikachu has no attack_s4_hi or attack_s4_lw motion at all, so both were cloned from
// attack_s4_s and played his STRAIGHT swing. Measured against Mac's real angled animations the
// up-angled hitboxes were up to 18.65 world units out of place, by far the worst error left.
//
// An adopted animation is a filename Pikachu does not have, so it must ALSO be shipped by
// tools/anim-dropin.sh and declared in the mod's config.json under new-dir-files
// "fighter/pikachu/c00", or ARCropolis will not load it.
use hash40::hash40;

// new entry | pikachu template entry | source fighter | source entry | adopt source animation
const SYNTH: &[(&str, &str, &str, &str, bool)] = &[
    // Game & Watch up-B (Fire + parachute). All four ADOPT his animations. They used to keep the
    // template's, which meant the up-B played Pikachu's QUICK ATTACK animations -- d02specialhi
    // start / d02specialairhistart and friends -- with G&W's `move` flag on top, so his own
    // teleport root motion was being applied. Reported as the up-B teleporting Pikachu extremely
    // high. G&W's four files are new names on Pikachu (d02specialhi, d02specialairhi,
    // d02specialhiopen, d02specialhiclose), so they need anim-dropin AND config.json entries.
    ("special_hi",       "special_hi_start",     "gamewatch", "special_hi",       true),
    ("special_air_hi",   "special_air_hi_start", "gamewatch", "special_air_hi",   true),
    ("special_hi_open",  "special_air_hi1",      "gamewatch", "special_hi_open",  true),
    ("special_hi_close", "special_air_hi_end",   "gamewatch", "special_hi_close", true),
    // Incineroar Revenge
    ("special_lw_start", "special_lw",           "gaogaen",   "special_lw_start", true),
    ("special_lw_turn",  "special_lw",           "gaogaen",   "special_lw_turn",  true),
    // AIR Revenge. Incineroar has special_air_lw, _start and _turn, and the air stance script is
    // timing-identical to the ground one -- same STANCE_START on 8 and STANCE_END on 28 -- so the
    // hand-written stance in src/specials/acmd_speciallw serves both. special_air_lw itself already
    // exists on Pikachu (air Thunder), so only these two are synthesised.
    ("special_air_lw_start", "special_air_lw", "gaogaen", "special_air_lw_start", true),
    ("special_air_lw_turn",  "special_air_lw", "gaogaen", "special_air_lw_turn",  true),
    // Incineroar Alolan Whip -- Pikachu has special_s_start and special_s_end already (Skull
    // Bash) but none of the three follow-ups or the whiff.
    ("special_s_shoulder", "special_s_start",     "gaogaen",   "special_s_shoulder", true),
    ("special_s_lariat",   "special_s_start",     "gaogaen",   "special_s_lariat",   true),
    ("special_s_throw",    "special_s_start",     "gaogaen",   "special_s_throw",    true),
    ("special_s_failure",  "special_s_start",     "gaogaen",   "special_s_failure",  true),
    // Alolan Whip has a full AIR variant -- six motions, six ACMDs and its own params
    // (special_air_s_catch_accel_y, _speed_y_max: he drifts slowly down while holding). The port
    // played the GROUNDED animation and grounded hitboxes for an airborne side-B until this was
    // found by reading his motion list rather than assuming the move had one form.
    // special_air_s_start and _end already exist on Pikachu (air Skull Bash), so motion-patch's MAP
    // gives those two Incineroar's flags and FAF; only these four are new.
    ("special_air_s_throw",    "special_air_s_start", "gaogaen", "special_air_s_throw",    true),
    ("special_air_s_shoulder", "special_air_s_start", "gaogaen", "special_air_s_shoulder", true),
    ("special_air_s_lariat",   "special_air_s_start", "gaogaen", "special_air_s_lariat",   true),
    ("special_air_s_failure",  "special_air_s_start", "gaogaen", "special_air_s_failure",  true),
    // LITTLE MAC'S JAB COMBO. Pikachu's jab is ONE hit -- he has `attack_11` and nothing after
    // it -- so jab 2, jab 3 and the whole rapid jab had no motion to change into and the playtest
    // saw exactly that: "little mac jab only has jab 1, it doesn't have access to jab 2, jab 3,
    // or rapid jab".
    //
    // All five adopt Mac's animations, and all five are filenames Pikachu does not have, so each
    // also needs a tools/anim-dropin.sh row (they get a config.json `new-dir-files` entry
    // automatically, because gen-config.sh derives that list from what is actually shipped).
    //
    // `attack_100_end`'s animation is `c00attackend.nuanmb`, NOT `c00attack100end.nuanmb`. The
    // entry name and the file name disagree in Mac's own motion list; adopting copies whatever he
    // names, so this needs no special case here -- but the anim-dropin row does have to ship that
    // exact stem or the entry points at a file the mod never put on the card.
    //
    // THE MOTIONS ARE ONLY HALF OF IT. Whether the engine ever ASKS for `attack_12` is decided by
    // fighter params, not by the motion list: `check_attack_mtrans` gates the combo on
    // `attack_combo_max`, which is 1 for Pikachu and 3 for Mac. See src/params/mod.rs.
    ("attack_12",        "attack_11",            "littlemac", "attack_12",        true),
    ("attack_13",        "attack_11",            "littlemac", "attack_13",        true),
    ("attack_100_start", "attack_11",            "littlemac", "attack_100_start", true),
    ("attack_100",       "attack_11",            "littlemac", "attack_100",       true),
    ("attack_100_end",   "attack_11",            "littlemac", "attack_100_end",   true),
    // Little Mac angled f-smash -- Pikachu has only attack_s4_s, so these adopt Mac's own
    // angled animations rather than playing his straight swing.
    ("attack_s4_hi",     "attack_s4_s",          "littlemac", "attack_s4_hi",     true),
    ("attack_s4_lw",     "attack_s4_s",          "littlemac", "attack_s4_lw",     true),
    // SAMUS'S CHARGE SHOT -- eleven motions, and Pikachu has NONE of them. His neutral-B is one
    // motion each way (`special_n`, `special_air_n`, Thunder Jolt); hers is a windup, a charge
    // loop, a cancel, a shot and a max-charge shot, ground and air, plus an air jump cancel.
    //
    // The templates are his two neutral-B entries, which is the closest thing he has and is only
    // used for the fields that are then overwritten -- `extra`, `flags`, `game_script`, `scripts`
    // and the animation all come from Samus.
    //
    // `special_n_f` and `special_air_n_f` ADOPT ANIMATION NAMES PIKACHU ALREADY HAS
    // (`d00specialn.nuanmb` / `d00specialairn.nuanmb`, which are hers and his both), so those two
    // are drop-in REPLACEMENTS rather than new files. The other nine are new paths and need their
    // config.json entries -- which gen-config.sh derives from what is shipped, so shipping them
    // in tools/anim-dropin.sh is the whole of it.
    //
    // `flags.loop` matters on the two hold entries and is carried by `flags`: the charge pose
    // loops for as long as the button is down, and an entry without it would play once and end,
    // which the status loop reads as "the animation finished".
    ("special_n_s",         "special_n",     "samus", "special_n_s",         true),
    ("special_n_h",         "special_n",     "samus", "special_n_h",         true),
    ("special_n_c",         "special_n",     "samus", "special_n_c",         true),
    ("special_n_f",         "special_n",     "samus", "special_n_f",         true),
    ("special_n_f_max",     "special_n",     "samus", "special_n_f_max",     true),
    ("special_air_n_s",     "special_air_n", "samus", "special_air_n_s",     true),
    ("special_air_n_h",     "special_air_n", "samus", "special_air_n_h",     true),
    ("special_air_n_c",     "special_air_n", "samus", "special_air_n_c",     true),
    ("special_air_n_f",     "special_air_n", "samus", "special_air_n_f",     true),
    ("special_air_n_f_max", "special_air_n", "samus", "special_air_n_f_max", true),
    ("special_air_n_jump_c","special_air_n", "samus", "special_air_n_jump_c",true),
    // DK cargo throws.
    ("throw_f_f",        "throw_f",              "donkey",    "throw_f_f",        true),
    ("throw_f_b",        "throw_b",              "donkey",    "throw_f_b",        true),
    ("throw_f_hi",       "throw_hi",             "donkey",    "throw_f_hi",       true),
    ("throw_f_lw",       "throw_lw",             "donkey",    "throw_f_lw",       true),
    // DK cargo CARRY -- the nine poses he uses while holding someone. The template for each is
    // Pikachu's own equivalent movement motion, because the carry keeps his locomotion (his walk
    // speeds, his jump) and changes only the pose; see src/specials/cargo/mod.rs.
    //
    // All nine animations were already on disk when this was written: the full c00 extraction
    // covers every fighter, so a feature that had been filed as "blocked on a console session"
    // turned out to be entirely local work.
    ("shoulder_wait",        "wait",          "donkey", "shoulder_wait",        true),
    ("shoulder_walk_slow",   "walk_slow",     "donkey", "shoulder_walk_slow",   true),
    ("shoulder_walk_middle", "walk_middle",   "donkey", "shoulder_walk_middle", true),
    ("shoulder_walk_fast",   "walk_fast",     "donkey", "shoulder_walk_fast",   true),
    ("shoulder_turn",        "turn",          "donkey", "shoulder_turn",        true),
    ("shoulder_jump_squat",  "jump_squat",    "donkey", "shoulder_jump_squat",  true),
    ("shoulder_jump",        "jump_f",        "donkey", "shoulder_jump",        true),
    ("shoulder_fall",        "fall",          "donkey", "shoulder_fall",        true),
    ("shoulder_landing",     "landing_light", "donkey", "shoulder_landing",     true),
];

fn main() {
    let ex = std::env::args().nth(1).expect("extracted root");
    let inp = std::env::args().nth(2).expect("input motion_list.bin");
    let out = std::env::args().nth(3).expect("output motion_list.bin");
    let mut pika = motion_lib::open(&inp).expect("open pikachu motion_list");

    let mut added = 0;
    for (new, template, fighter, src_entry, adopt_anim) in SYNTH {
        let nk = hash40(new);
        if pika.list.contains_key(&nk) {
            println!("  -- {new:<20} already present, skipped");
            continue;
        }
        let Some(tpl) = pika.list.get(&hash40(template)).cloned() else {
            println!("  !! {new:<20} template '{template}' missing on pikachu");
            continue;
        };
        let src = motion_lib::open(format!("{ex}/fighter/{fighter}/motion/body/c00/motion_list.bin"))
            .unwrap_or_else(|e| panic!("open {fighter}: {e:?}"));
        let Some(s) = src.list.get(&hash40(src_entry)) else {
            println!("  !! {new:<20} '{src_entry}' missing on {fighter}");
            continue;
        };
        let mut m = tpl;
        m.extra = s.extra.clone();
        m.flags = s.flags;
        // Take the source fighter's game_script too. Without this the new entry keeps the
        // template's script name and any ACMD we register against the source name is dead.
        m.game_script = s.game_script;
        // AND ITS `scripts`, WHICH THIS DID NOT DO -- THE BUG THAT MADE THE UP-B INVISIBLE.
        //
        // `scripts` is the expression / sound / effect trio, and it was left pointing at the
        // TEMPLATE's. So `special_hi_open`, cloned from one of Pikachu's Quick Attack entries,
        // named `expression_specialairhi1` -- and Pikachu's SpecialAirHi1 expression script calls
        // `VisibilityModule::set_whole(false)`. The up-B's own status logic was correct all along
        // and measured working on hardware (special_hi -> _open -> _close, +74.98 units of rise);
        // he was simply hidden by his own script running under our animation.
        //
        // Thirty-eight of seventy-seven entries were like this, which is also where the "quick
        // attack arrows" came from -- `effect_specialhistart` under G&W's Fire -- and it put
        // Pikachu's THUNDER expression and effect scripts on Revenge's stance.
        //
        // Same reasoning as game_script, and it should always have been the same line: a
        // synthesised entry that keeps ANY of the template's script names runs Pikachu's code for
        // a move that is no longer Pikachu's. Where we register nothing under the source's name
        // the script is simply absent, which is a no-op -- strictly better than the wrong script.
        m.scripts = s.scripts.clone();
        // Same reasoning as motion-patch MAP: the source's cross-fade into the move is part of the
        // move. A synthesised entry inherits the template's otherwise.
        m.blend_frames = s.blend_frames;
        let mut anim_note = String::new();
        if *adopt_anim {
            m.animations = s.animations.clone();
            anim_note = format!(" anim<-{fighter}");
        }
        let f = match &m.extra {
            Some(x) => format!("faf {:>3} xlu {}-{}", x.cancel_frame, x.xlu_start, x.xlu_end),
            None => "none".into(),
        };
        pika.list.insert(nk, m);
        println!("  ok {new:<20} <- template {template:<22} {f}{anim_note}  ({fighter}/{src_entry})");
        added += 1;
    }
    motion_lib::save(&out, &pika).expect("save");
    println!("\nadded {added} motion entries; wrote {out}");
}
