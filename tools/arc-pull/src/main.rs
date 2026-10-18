use smash_arc::{ArcFile, ArcLookup, Hash40, Region};
use std::io::{BufRead, Write};

fn main() {
    let mut args = std::env::args().skip(1);
    let addr = args.next().unwrap_or_else(arc_pull::default_addr);
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
                // A COLON IS A LEGAL ARC PATH AND AN ILLEGAL WINDOWS ONE.
                //
                // Every path pulled so far has been `fighter/...`, so this never came up. The
                // lua2cpp agent NROs live under `prebuilt:/nro/release/`, and joining that
                // verbatim produces `out\prebuilt:\nro\release\...`, which Windows rejects with
                // "the filename, directory name, or volume label syntax is incorrect" -- an
                // error that reads like a network or permissions problem and is neither.
                //
                // The colon is the arc's namespace separator, so a slash is the faithful
                // replacement: `prebuilt:/nro` becomes `prebuilt/nro`.
                let safe: String = path.replace(':', "");
                let dest = std::path::Path::new(&outdir).join(&safe);
                std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
                let mut f = std::fs::File::create(&dest).unwrap();
                f.write_all(&data).unwrap();
                println!("OK    {:>10}  {}", data.len(), path);
            }
            Err(e) => println!("FAIL              {}  ({e:?})", path),
        }
    }
}
