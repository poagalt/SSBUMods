use hash40::hash40;
fn main() {
    for a in std::env::args().skip(1) {
        println!("{:#014x}  {}", hash40(&a).0, a);
    }
}
