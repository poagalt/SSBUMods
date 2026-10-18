// Pulls `stream:/...` files -- the BGM and its banks -- which `arc-pull` cannot reach.
//
// WHY A SEPARATE TOOL. Stream files are not in the archive's file tree at all: they live in their
// own section with their own hash table (`StreamEntry` -> file index -> `StreamData`), are stored
// uncompressed, and are absent from `work/acquire/arc-paths.txt` for the same reason. smash-arc
// already reads them; this only asks it to, over the same arc_network pipe, so it needs the game
// running like every other arc tool.
//
// usage: arc-stream <out-dir> <stream:/path> [<stream:/path> ...]
//        (the console address comes from tools/console.sh's file, as for arc-pull)
use smash_arc::{ArcFile, ArcLookup, Hash40, Region};

fn main() {
    let mut a = std::env::args().skip(1);
    let out = a.next().expect("usage: arc-stream <out-dir> <stream:/path>...");
    let paths: Vec<String> = a.collect();
    if paths.is_empty() {
        eprintln!("usage: arc-stream <out-dir> <stream:/path>...");
        std::process::exit(2);
    }
    let addr = arc_pull::default_addr();
    eprintln!("connecting to {addr} ...");
    let arc = ArcFile::open_over_network(addr.as_str()).expect("failed to open arc over network");
    std::fs::create_dir_all(&out).expect("out dir");
    let mut failed = 0;
    for p in &paths {
        match arc.get_stream_file_contents(Hash40::from(p.as_str()), Region::UsEnglish) {
            Ok(bytes) => {
                let name = p.rsplit('/').next().unwrap();
                let dest = std::path::Path::new(&out).join(name);
                std::fs::write(&dest, &bytes).expect("write");
                println!("  ok  {p}  {} bytes -> {}", bytes.len(), dest.display());
            }
            Err(e) => {
                println!("  FAIL {p}: {e:?}");
                failed += 1;
            }
        }
    }
    if failed > 0 {
        std::process::exit(1);
    }
}
