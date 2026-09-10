// Patches Pikachu's motion_list.bin with FAF (cancel_frame), intangibility
// (xlu_start / xlu_end) and animation flags from each move's source fighter.
//
// Only `extra` and `flags` are copied. `animations`, `game_script` and `scripts` stay
// Pikachu's -- those point at Pikachu's own animation files and script names.
use hash40::hash40;
use std::collections::BTreeMap;

// motion entry -> source fighter
const MAP: &[(&str, &str, &str)] = &[
    // normals
    ("attack_11", "attack_11", "chrom"),
    ("attack_dash", "attack_dash", "elight"),
    ("attack_s3_s", "attack_s3_s", "elight"), ("attack_s3_hi", "attack_s3_s", "elight"), ("attack_s3_lw", "attack_s3_s", "elight"),
    ("attack_hi3", "attack_hi3", "falco"),
    ("attack_lw3", "attack_lw3", "robot"),
    // smashes
    ("attack_s4_s", "attack_s4_s", "littlemac"),     ("attack_s4_hold", "attack_s4_hold", "littlemac"),
    ("attack_hi4", "attack_hi4", "fox"), ("attack_hi4_hold", "attack_hi4_hold", "fox"),
    ("attack_lw4", "attack_lw4", "gamewatch"), ("attack_lw4_hold", "attack_lw4_hold", "gamewatch"),
    // aerials + their landing lag
    ("attack_air_n", "attack_air_n", "palutena"), ("landing_air_n", "landing_air_n", "palutena"),
    ("attack_air_f", "attack_air_f", "demon"),    ("landing_air_f", "landing_air_f", "demon"),
    ("attack_air_b", "attack_air_b", "cloud"),    ("landing_air_b", "landing_air_b", "cloud"),
    ("attack_air_hi", "attack_air_hi", "kamui"),   ("landing_air_hi", "landing_air_hi", "kamui"),
    ("attack_air_lw", "attack_air_lw", "krool"),   ("landing_air_lw", "landing_air_lw", "krool"),
    // grab / throws
    ("catch", "catch", "palutena"), ("catch_dash", "catch_dash", "palutena"), ("catch_turn", "catch_turn", "palutena"),
    ("catch_wait", "catch_wait", "palutena"), ("catch_pull", "catch_pull", "palutena"), ("catch_cut", "catch_cut", "palutena"),
    ("catch_attack", "catch_attack", "demon"),
    ("throw_b", "throw_b", "gaogaen"),
    ("throw_hi", "throw_hi", "mewtwo"),
    // defensive
    ("down_attack_u", "down_attack_u", "koopa"), ("down_attack_d", "down_attack_d", "koopa"), ("slip_attack", "slip_attack", "koopa"),
    ("cliff_attack_quick", "cliff_attack_quick", "ganon"),
    // Fox dodges
    ("escape", "escape", "fox"), ("escape_f", "escape_f", "fox"), ("escape_b", "escape_b", "fox"), ("escape_air", "escape_air", "fox"),
    ("escape_air_slide", "escape_air_slide", "fox"),
];

fn main() {
    let ex = std::env::args().nth(1).expect("usage: motion-patch <extracted-root> <out.bin>");
    let out = std::env::args().nth(2).expect("out path");

    let pika_path = format!("{ex}/fighter/pikachu/motion/body/c00/motion_list.bin");
    let mut pika = motion_lib::open(&pika_path).expect("open pikachu motion_list");

    let mut cache: BTreeMap<String, motion_lib::mlist::MList> = BTreeMap::new();
    let (mut done, mut missing_src, mut missing_dst) = (0, 0, 0);

    for (entry, src_entry, fighter) in MAP {
        let src = cache.entry(fighter.to_string()).or_insert_with(|| {
            motion_lib::open(format!("{ex}/fighter/{fighter}/motion/body/c00/motion_list.bin"))
                .unwrap_or_else(|e| panic!("open {fighter}: {e:?}"))
        });
        let key = hash40(entry);
        let src_key = hash40(src_entry);
        let Some(s) = src.list.get(&src_key).cloned() else {
            println!("  -- {entry:<22} not present on {fighter}");
            missing_src += 1;
            continue;
        };
        let Some(d) = pika.list.get_mut(&key) else {
            println!("  -- {entry:<22} not present on pikachu");
            missing_dst += 1;
            continue;
        };
        let before = d.extra.clone();
        d.extra = s.extra.clone();
        d.flags = s.flags;
        let f = |e: &Option<motion_lib::mlist::Extra>| match e {
            Some(x) => format!("faf {:>3} xlu {}-{}", x.cancel_frame, x.xlu_start, x.xlu_end),
            None => "none".into(),
        };
        println!("  ok {entry:<22} {:<28} -> {:<28} ({fighter})", f(&before), f(&d.extra));
        done += 1;
    }

    motion_lib::save(&out, &pika).expect("save");
    println!("\npatched {done}, source-missing {missing_src}, pikachu-missing {missing_dst}");
    println!("wrote {out}");
}
