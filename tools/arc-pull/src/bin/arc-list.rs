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
    let labels = HashLabels::from_file("vendor/Hashes.txt").unwrap_or_else(|e| {
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
        let h: Hash40 = path.path.hash40();
        match h.label(&labels) {
            Some(s) => println!("{s}"),
            None => println!("0x{:010x}", h.as_u64()),
        }
        n += 1;
    }
    eprintln!("{n} path(s)");
}
