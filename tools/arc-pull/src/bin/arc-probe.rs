// Diagnostic: can a file's path be REASSEMBLED from its parent and file_name hashes?
//
// vendor/Hashes.txt holds path COMPONENTS -- 89,059 directory paths and the bare filenames -- not
// full paths. So `FilePath::path`, which is the hash of the whole path, resolves for almost
// nothing (151 of 590,711, all of them `prebuilt:/` fonts). But every FilePath also carries
// `parent`, `file_name` and `ext` as separate hashes, and those are components. If they resolve,
// `parent/file_name` reconstructs the path with no full-path dictionary needed -- and the download
// can still use `path`'s hash directly, so nothing has to be re-hashed and nothing is guessed.
//
// usage: arc-probe <ip:port> [sample-count]
use smash_arc::{ArcFile, ArcLookup, Hash40};

fn main() {
    let mut a = std::env::args().skip(1);
    let addr = a.next().unwrap_or_else(arc_pull::default_addr);
    let sample: usize = a.next().and_then(|s| s.parse().ok()).unwrap_or(20);

    let labels = ["vendor/Hashes.txt", "../../vendor/Hashes.txt"]
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .expect("no Hashes.txt")
        .to_string();
    Hash40::set_global_labels_file(&labels).expect("labels");

    let arc = ArcFile::open_over_network(addr.as_str()).expect("open arc");
    eprintln!("arc opened.");

    let (mut both, mut parent_only, mut name_only, mut neither) = (0u64, 0u64, 0u64, 0u64);
    let mut shown = 0usize;

    for fp in arc.get_file_paths() {
        let p = fp.parent.hash40().global_label();
        let n = fp.file_name.hash40().global_label();
        match (&p, &n) {
            (Some(_), Some(_)) => both += 1,
            (Some(_), None) => parent_only += 1,
            (None, Some(_)) => name_only += 1,
            (None, None) => neither += 1,
        }
        if shown < sample {
            if let (Some(p), Some(n)) = (p, n) {
                if p.starts_with("fighter/") {
                    println!("  {p}/{n}");
                    shown += 1;
                }
            }
        }
    }

    let total = both + parent_only + name_only + neither;
    println!();
    println!("{total} paths");
    println!("  parent + file_name both resolve : {both}");
    println!("  parent only                     : {parent_only}");
    println!("  file_name only                  : {name_only}");
    println!("  neither                         : {neither}");
}
