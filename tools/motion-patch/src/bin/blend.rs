// Compares `blend_frames` between Pikachu's motion entry and the source fighter's.
//
// blend_frames is how many frames the engine cross-fades from the previous pose into this
// motion. It is the field that governs how a move feels coming IN, and motion-patch has only
// ever copied `extra` and `flags` -- so every ported move has been blending on Pikachu's timing
// while playing the source's animation.
use hash40::hash40;
fn main() {
    let mut a = std::env::args().skip(1);
    let ex = a.next().unwrap();
    let ours_path = a.next().unwrap();
    let pk = motion_lib::open(format!("{ex}/fighter/pikachu/motion/body/c00/motion_list.bin")).unwrap();
    let ours = motion_lib::open(&ours_path).unwrap();
    // REFUSE TO REPORT CLEAN ON ZERO WORK. This tool takes its motions as `fighter:motion` args,
    // and run with none it printed an empty table and "0 motion(s) blending on the wrong timing" --
    // which is exactly how a check earns trust it has not done anything to deserve. That pattern
    // has produced five false cleans on this project already. Given no specs, it is a usage error.
    let specs: Vec<String> = a.collect();
    if specs.is_empty() {
        eprintln!("no motions given -- usage: blend <extracted> <ours.bin> <fighter>:<motion>...");
        eprintln!("a run with nothing to check is a usage error, not a pass");
        std::process::exit(2);
    }

    println!("{:<20} {:<10} {:>8} {:>8} {:>8}", "motion", "source", "pikachu", "source", "ours");
    let mut diff = 0;
    let mut missing = 0;
    for spec in specs {
        let (fighter, motion) = spec.split_once(':').unwrap();
        let h = hash40(motion);
        let sm = motion_lib::open(format!("{ex}/fighter/{fighter}/motion/body/c00/motion_list.bin")).unwrap();
        let (Some(s), Some(p), Some(o)) = (sm.list.get(&h), pk.list.get(&h), ours.list.get(&h)) else {
            println!("{motion:<20} {fighter:<10}  -- not on all three (synthesised, or the source lacks it)");
            missing += 1; continue };
        let mark = if s.blend_frames != o.blend_frames { diff += 1; "  <-- ours is Pikachu's" } else { "" };
        println!("{motion:<20} {fighter:<10} {:>8} {:>8} {:>8}{mark}",
                 p.blend_frames, s.blend_frames, o.blend_frames, mark = mark);
    }
    println!("\n{diff} motion(s) blending on the wrong timing, {missing} not comparable");
    // Non-zero so check.sh can gate on it rather than on grepping the text.
    if diff > 0 {
        std::process::exit(1);
    }
}
