use smash_arc::{ArcFile, ArcLookup, Hash40, Region};
use std::io::{BufRead, Write};

fn main() {
    let mut args = std::env::args().skip(1);
    let addr = args.next().unwrap_or_else(|| "<console-ip>:43022".into());
    let outdir = args.next().unwrap_or_else(|| "out".into());

    eprintln!("connecting to {addr} ...");
    let arc = ArcFile::open_over_network(addr.as_str()).expect("failed to open arc over network");
    eprintln!("arc opened.");

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let path = line.unwrap();
        let path = path.trim();
        if path.is_empty() || path.starts_with('#') {
            continue;
        }
        let hash = Hash40::from(path);
        match arc.get_file_contents(hash, Region::UsEnglish) {
            Ok(data) => {
                let dest = std::path::Path::new(&outdir).join(path);
                std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
                let mut f = std::fs::File::create(&dest).unwrap();
                f.write_all(&data).unwrap();
                println!("OK    {:>10}  {}", data.len(), path);
            }
            Err(e) => println!("FAIL              {}  ({e:?})", path),
        }
    }
}
