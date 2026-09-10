use hash40::hash40;
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let m = motion_lib::open(&p).expect("PARSE FAILED");
    println!("parsed OK: {} entries", m.list.len());
    for n in ["special_hi","special_air_hi","special_hi_open","special_hi_close",
              "special_lw_start","special_lw_turn",
              "throw_f_f","throw_f_b","throw_f_hi","throw_f_lw",
              "attack_hi4","attack_lw4","cliff_attack_quick","escape_air_slide"] {
        let e = m.list.get(&hash40(n));
        println!("  {:<20} {}", n, match e {
            Some(x) => match &x.extra { Some(v) => format!("present  faf {:>3} xlu {}-{}", v.cancel_frame, v.xlu_start, v.xlu_end), None => "present  (no extra)".into() },
            None => "MISSING".into(),
        });
    }
}
