// Adds motion entries that Pikachu lacks, so ported special-move status scripts have
// something to MotionModule::change_motion into.
//
// Each new entry is cloned from an existing Pikachu entry (so `animations` and `scripts`
// point at real Pikachu files that certainly load), then given the source fighter's
// `extra` (FAF + intangibility) and `flags`. Consistent with the script-first decision:
// correct timing, Pikachu's animation.
use hash40::hash40;

// new entry | pikachu template entry | source fighter | source entry
const SYNTH: &[(&str, &str, &str, &str)] = &[
    // Game & Watch up-B (Fire + parachute)
    ("special_hi",        "special_hi_start",     "gamewatch", "special_hi"),
    ("special_air_hi",    "special_air_hi_start", "gamewatch", "special_air_hi"),
    ("special_hi_open",   "special_air_hi1",      "gamewatch", "special_hi_open"),
    ("special_hi_close",  "special_air_hi_end",   "gamewatch", "special_hi_close"),
    // Incineroar Revenge
    ("special_lw_start",  "special_lw",           "gaogaen",   "special_lw_start"),
    ("special_lw_turn",   "special_lw",           "gaogaen",   "special_lw_turn"),
    // Little Mac angled f-smash -- Pikachu has only attack_s4_s
    ("attack_s4_hi",     "attack_s4_s",          "littlemac", "attack_s4_hi"),
    ("attack_s4_lw",     "attack_s4_s",          "littlemac", "attack_s4_lw"),
    // DK cargo throws
    ("throw_f_f",         "throw_f",              "donkey",    "throw_f_f"),
    ("throw_f_b",         "throw_b",              "donkey",    "throw_f_b"),
    ("throw_f_hi",        "throw_hi",             "donkey",    "throw_f_hi"),
    ("throw_f_lw",        "throw_lw",             "donkey",    "throw_f_lw"),
];

fn main() {
    let ex = std::env::args().nth(1).expect("extracted root");
    let inp = std::env::args().nth(2).expect("input motion_list.bin");
    let out = std::env::args().nth(3).expect("output motion_list.bin");
    let mut pika = motion_lib::open(&inp).expect("open pikachu motion_list");

    let mut added = 0;
    for (new, template, fighter, src_entry) in SYNTH {
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
        let f = match &m.extra {
            Some(x) => format!("faf {:>3} xlu {}-{}", x.cancel_frame, x.xlu_start, x.xlu_end),
            None => "none".into(),
        };
        pika.list.insert(nk, m);
        println!("  ok {new:<20} <- template {template:<22} {f}  ({fighter}/{src_entry})");
        added += 1;
    }
    motion_lib::save(&out, &pika).expect("save");
    println!("\nadded {added} motion entries; wrote {out}");
}
