// Every path in the game's archive, so "is it in the data we have" stops being a guess.
//
// WHY. The project had 97 fighters' worth of extracted assets and no way to say what that was 97
// out of. `tools/arc-pull` fetches paths you already know; nothing enumerated them, so a file
// nobody thought to ask for was indistinguishable from a file that does not exist -- which is
// the same shape as the five-round cargo bug, where the deciding code was not missing but
// unreadable.
//
// `smash-arc`'s `dir-listing` feature walks the archive's own directory tree, so this is the
// game's answer rather than ours. Needs the game running: the arc is served over the network by
// the arc-network plugin inside the process.
use smash_arc::{ArcFile, ArcLookup, Hash40, HashLabels};

fn main() {
    let addr = std::env::args().nth(1).unwrap_or_else(|| "<console-ip>:43022".into());
    eprintln!("connecting to {addr} ...");
    let arc = ArcFile::open_over_network(addr.as_str()).expect("failed to open arc over network");
    // The archive stores hashes, not strings. `vendor/Hashes.txt` is the community name list
    // this repo already ships for `tools/hash40.sh`, so the same list names the paths here.
    let labels = HashLabels::from_file("vendor/ArcHashes.txt").unwrap_or_else(|e| {
        eprintln!("no vendor/Hashes.txt ({e}); paths will print as hashes");
        HashLabels::from_string("")
    });
    eprintln!("arc opened; walking every file path");

    let mut n = 0usize;
    for path in arc.get_file_paths() {
        // `get_file_paths` yields the hash40 of each path. The archive stores its own strings, so
        // `global_labels` resolves the ones it knows and the rest print as their hash -- which is
        // still a usable key for `arc-pull`.
        // `FilePath.path` is the full path, as a `HashToIndex` whose `hash40()` is the key
        // `arc-pull` takes.
        // THE ARCHIVE DOES NOT STORE THE FULL PATH STRING, AND HASHING IT BACK DOES NOT WORK.
        //
        // Printing `label(path)` resolved 151 of 590,711: the community lists hold DIRECTORIES
        // (`fighter/mario/model/body/c00`) and FILENAMES (`model.numdlb`), not the ~590k full
        // paths, and a full-path hash has no entry to match. The archive itself keeps `parent`
        // and `file_name` as separate hashes for exactly this reason, so joining the two labels
        // reconstructs the path from parts that ARE named.
        let parent = path.parent.hash40();
        let name = path.file_name.hash40();
        match (parent.label(&labels), name.label(&labels)) {
            (Some(d), Some(f)) if d.is_empty() => println!("{f}"),
            // The directory label already carries its trailing slash.
            (Some(d), Some(f)) => println!("{}/{f}", d.trim_end_matches('/')),
            // Either half unnamed: print what is known plus the raw hash, so the line is still a
            // usable key for `arc-pull` and still says which part is missing.
            (Some(d), None) => println!("{}/0x{:010x}", d.trim_end_matches('/'), name.as_u64()),
            (None, Some(f)) => println!("0x{:010x}/{f}", parent.as_u64()),
            (None, None) => println!("0x{:010x}", path.path.hash40().as_u64()),
        }
        n += 1;
    }
    eprintln!("{n} path(s)");
}
