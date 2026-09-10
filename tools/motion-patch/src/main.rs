// Patches Pikachu's motion_list.bin with FAF (cancel_frame), intangibility
// (xlu_start / xlu_end) and animation flags from each move's source fighter.
//
// Only `extra` and `flags` are copied by MAP. `animations`, `game_script` and `scripts` stay
// Pikachu's -- those point at Pikachu's own animation files and script names. REANIM below is the
// one exception, and it only ever repoints an entry at ANOTHER PIKACHU ENTRY's file, so it can
// never name a path the mod does not ship.
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
    // Specials whose motion Pikachu ALREADY has, so synth skips them and only motion-patch can
    // give them the source's flags and FAF. Missing this cost Alolan Whip its entire lunge:
    // Incineroar's d01specialsstart carries 34.93 units of root motion in Trans, which the engine
    // only applies when the motion entry has the `move` flag -- and ours did not, so the command
    // grab never travelled and could never connect. tools/motion-patch dropin had been reporting
    // exactly this, but the build's pre-flight list did not include these motions.
    ("special_s_start", "special_s_start", "gaogaen"), ("special_s_end", "special_s_end", "gaogaen"),
    // Air Alolan Whip's two pre-existing entries. Pikachu has special_air_s_start and _end from
    // air Skull Bash, so they need Incineroar's flags and FAF rather than synthesis.
    ("special_air_s_start", "special_air_s_start", "gaogaen"),
    ("special_air_s_end", "special_air_s_end", "gaogaen"),
    ("special_lw", "special_lw", "gaogaen"),
    ("special_air_lw", "special_air_lw", "gaogaen"),
    // grab / throws
    ("catch", "catch", "palutena"), ("catch_dash", "catch_dash", "palutena"), ("catch_turn", "catch_turn", "palutena"),
    ("catch_wait", "catch_wait", "palutena"), ("catch_pull", "catch_pull", "palutena"), ("catch_cut", "catch_cut", "palutena"),
    ("catch_attack", "catch_attack", "demon"),
    // The cargo GRAB. Paired deliberately with the anim-dropin entry for e01throwf.nuanmb: the
    // flags and the animation have to arrive together, or this entry takes DK's `move` flag over
    // Pikachu's own f-throw animation and applies his root motion to the wrong pose.
    ("throw_f", "throw_f", "donkey"),
    ("throw_b", "throw_b", "gaogaen"),
    ("throw_hi", "throw_hi", "mewtwo"),
    // defensive
    ("down_attack_u", "down_attack_u", "koopa"), ("down_attack_d", "down_attack_d", "koopa"), ("slip_attack", "slip_attack", "koopa"),
    ("cliff_attack_quick", "cliff_attack_quick", "ganon"),
    // Fox dodges
    ("escape", "escape", "fox"), ("escape_f", "escape_f", "fox"), ("escape_b", "escape_b", "fox"), ("escape_air", "escape_air", "fox"),
    ("escape_air_slide", "escape_air_slide", "fox"),
];

// Entries whose ANIMATION must be repointed at another of Pikachu's entries.
//
// A source fighter does not necessarily have a motion for every variant Pikachu has. Mythra has
// NO attack_s3_hi and NO attack_s3_lw at all -- her f-tilt is one animation for all three angles,
// which is why gen-moves.sh aliases both angled ACMDs to her attacks3 script. Pikachu, though,
// does have c01attacks3hi.nuanmb and c01attacks3lw.nuanmb, and nothing was overriding them, so
// two of the three angles kept playing HIS tail-whip: reported in playtest as "pikachu's down and
// up angled ftilt animations still exist".
//
// Pointing both at attack_s3_s's file makes every angle play Mythra's single animation, matching
// what she actually does. That file is already dropped in by tools/anim-dropin.sh, so this needs
// no new shipped path and no config.json entry.
const REANIM: &[(&str, &str)] = &[
    ("attack_s3_hi", "attack_s3_s"),
    ("attack_s3_lw", "attack_s3_s"),
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
        // blend_frames is how many frames the engine cross-fades from the previous pose INTO this
        // motion -- so it is literally how a move feels coming in, and it had never been copied.
        // Measured across all 48 ported motions, exactly one disagreed: Palutena's catch_dash
        // blends over 3 frames and ours snapped in over 0. One instance, but the whole class is
        // fixed by copying the field, and tools/motion-patch/src/bin/mlparity.rs now runs in check.sh
        // so it cannot drift back.
        d.blend_frames = s.blend_frames;
        let f = |e: &Option<motion_lib::mlist::Extra>| match e {
            Some(x) => format!("faf {:>3} xlu {}-{}", x.cancel_frame, x.xlu_start, x.xlu_end),
            None => "none".into(),
        };
        println!("  ok {entry:<22} {:<28} -> {:<28} ({fighter})", f(&before), f(&d.extra));
        done += 1;
    }

    // Animation repoints, after the flag/FAF pass so a repointed entry keeps its source's timing.
    // Both sides must exist on Pikachu; a typo here would otherwise silently do nothing, so this
    // panics rather than warning.
    let mut reanimed = 0;
    for (entry, from) in REANIM {
        let src_anims = match pika.list.get(&hash40(from)) {
            Some(e) => e.animations.clone(),
            None => panic!("REANIM source '{from}' is not a pikachu motion entry"),
        };
        match pika.list.get_mut(&hash40(entry)) {
            Some(d) => {
                d.animations = src_anims;
                println!("  ok {entry:<22} animation <- {from}");
                reanimed += 1;
            }
            None => panic!("REANIM target '{entry}' is not a pikachu motion entry"),
        }
    }

    motion_lib::save(&out, &pika).expect("save");
    println!("\npatched {done}, reanimed {reanimed}, source-missing {missing_src}, pikachu-missing {missing_dst}");
    println!("wrote {out}");
}
