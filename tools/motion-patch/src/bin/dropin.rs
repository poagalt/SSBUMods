// Compares a ported motion's entry between the SOURCE fighter and Pikachu.
//
// This is the pre-flight check for the raw-animation drop-in: Pikachu plays the source's own
// .nuanmb, so his motion entry must agree with the source's about how that animation is driven.
// The flags that matter are `move` (honour the animation's root motion) and `fix_trans`/`fix_rot`/
// `fix_scale` (pin those tracks to bind, i.e. suppress them). Little Mac's f-smash lunge lives in
// a Trans track, so it appears in game only if Pikachu's entry has `move` set and `fix_trans` clear.
//
// usage: dropin <extracted-root> <patched-motion-list|-> <fighter:motion>...
use hash40::hash40;

fn fl(f: &motion_lib::mlist::Flags) -> String {
    let mut v = vec![];
    if f.turn { v.push("turn") }
    if f.r#loop { v.push("loop") }
    if f.r#move { v.push("MOVE") }
    if f.fix_trans { v.push("fix_trans") }
    if f.fix_rot { v.push("fix_rot") }
    if f.fix_scale { v.push("fix_scale") }
    if v.is_empty() { "-".into() } else { v.join("+") }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let ex = a.next().unwrap();
    let patched = a.next().unwrap();

    let pk = motion_lib::open(format!("{ex}/fighter/pikachu/motion/body/c00/motion_list.bin")).unwrap();
    let ours = if patched == "-" { None } else { motion_lib::open(&patched).ok() };

    println!("{:<14} {:<14} {:<22} {:<22} {:<22} {:>6} {:>6}",
             "motion", "source", "SOURCE flags", "pikachu vanilla", "OURS", "s.faf", "o.faf");
    let mut warn = 0;
    for spec in a {
        let (fighter, motion) = spec.split_once(':').unwrap();
        let h = hash40(motion);
        let sm = motion_lib::open(format!("{ex}/fighter/{fighter}/motion/body/c00/motion_list.bin")).unwrap();
        let Some(s) = sm.list.get(&h) else { println!("{motion:<14} {fighter:<14} -- not in source"); continue };
        // A SYNTHESISED entry has no vanilla Pikachu row -- attack_s4_hi and attack_s4_lw exist
        // only because tools/motion-patch synth added them. Refusing to report those left the
        // inventory showing a blank FAF for two moves whose FAF is in fact correct, which reads
        // as a gap that is not there. Report them with the vanilla column marked absent.
        let p = pk.list.get(&h);
        let o = ours.as_ref().and_then(|m| m.list.get(&h));

        let sfaf = s.extra.as_ref().map(|e| e.cancel_frame as i32).unwrap_or(-1);
        let ofaf = o.and_then(|o| o.extra.as_ref()).map(|e| e.cancel_frame as i32).unwrap_or(-1);
        println!("{:<14} {:<14} {:<22} {:<22} {:<22} {:>6} {:>6}",
                 motion, fighter, fl(&s.flags),
                 p.map(|p| fl(&p.flags)).unwrap_or_else(|| "(synthesised)".into()),
                 o.map(|o| fl(&o.flags)).unwrap_or_else(|| "(none)".into()), sfaf, ofaf);

        // The animation filename is universal per motion in SSBU, so a drop-in is a plain
        // same-name file replacement. Verify rather than assume.
        if let Some(p) = p {
            let sa: Vec<u64> = s.animations.iter().map(|x| x.name.0).collect();
            let pa: Vec<u64> = p.animations.iter().map(|x| x.name.0).collect();
            if sa != pa { println!("    ! animation names DIFFER -- a plain copy will not work"); warn += 1; }
        }
        if let Some(o) = o {
            if s.flags.r#move && !o.flags.r#move {
                println!("    ! source has root motion but ours does not set `move` -- lunge will be dropped"); warn += 1;
            }
            if !s.flags.fix_trans && o.flags.fix_trans {
                println!("    ! ours sets fix_trans -- the animation's Trans track will be ignored"); warn += 1;
            }
        }
    }
    println!("\n{warn} warnings");
}
