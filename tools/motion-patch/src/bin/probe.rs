use hash40::hash40;
fn main() {
    let mut a = std::env::args().skip(1);
    let ex = a.next().unwrap();
    let fighter = a.next().unwrap();
    let m = motion_lib::open(format!("{ex}/fighter/{fighter}/motion/body/c00/motion_list.bin")).unwrap();
    for name in a {
        println!("{:<28} {}", name, if m.list.contains_key(&hash40(&name)) { "present" } else { "-" });
    }
}
