// Lists every file in the game's data.arc with its decompressed size, WITHOUT downloading any of
// it, and optionally writes a path list for arc-pull to consume.
//
// WHY THIS EXISTS. Nine files were lost in one extraction round because their names were invented
// -- `h00speciallw...` instead of the real `d03speciallw...`. animof.exe fixed that for animations
// by reading a fighter's own motion_list, but only for motions we already knew to ask about. This
// reads the arc's file table itself: `get_file_paths()` is the complete flat list of every path in
// the archive, so nothing has to be guessed or known in advance.
//
// It is also how a bulk pull gets sized before it is started. Every FileData carries comp_size and
// decomp_size, so the cost of "extract all of X" is a table read, not a download.
//
// usage:
//   arc-list <ip:port> [--prefix P]... [--out FILE] [--quiet]
//
//   --prefix   keep only paths starting with P. Repeatable; a path matching ANY prefix is kept.
//              With none, every path in the arc is listed.
//   --out      write the matching paths, one per line, ready for:
//                  arc-pull <ip:port> <outdir> < FILE
//   --quiet    totals only, no per-file lines.
//
// Sizes are reported for Region::UsEnglish, the same region arc-pull downloads.
use smash_arc::{ArcFile, ArcLookup, Hash40, Region};
use std::io::Write;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    let quiet = args.iter().any(|a| a == "--quiet");
    args.retain(|a| a != "--quiet");

    let mut prefixes: Vec<String> = Vec::new();
    let mut out: Option<String> = None;
    let mut positional: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--prefix" => {
                i += 1;
                prefixes.push(args.get(i).expect("--prefix needs a value").clone());
            }
            "--out" => {
                i += 1;
                out = Some(args.get(i).expect("--out needs a value").clone());
            }
            other => positional.push(other.to_string()),
        }
        i += 1;
    }
    let addr = positional
        .into_iter()
        .next()
        .unwrap_or_else(|| "<console-ip>:43022".into());

    // The arc stores hashes, not names. Without the global label set every path resolves to a bare
    // hash and the listing is useless, so this is a hard requirement rather than a nicety.
    let labels = ["vendor/Hashes.txt", "../../vendor/Hashes.txt", "Hashes.txt"]
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .map(|p| p.to_string())
        .expect("no Hashes.txt found -- looked in vendor/, ../../vendor/ and .");
    Hash40::set_global_labels_file(&labels).expect("failed to load hash labels");
    eprintln!("labels: {labels}");

    eprintln!("connecting to {addr} ...");
    let arc = ArcFile::open_over_network(addr.as_str()).expect("failed to open arc over network");
    eprintln!("arc opened.");

    let mut writer = out.as_ref().map(|p| {
        std::io::BufWriter::new(std::fs::File::create(p).expect("cannot create --out file"))
    });

    let (mut kept, mut kept_bytes) = (0u64, 0u64);
    let (mut total, mut unnamed) = (0u64, 0u64);

    for fp in arc.get_file_paths() {
        total += 1;
        let hash = fp.path.hash40();
        // A path whose hash is not in Hashes.txt cannot be written to disk under a real name, so
        // it is counted and skipped rather than saved as a hex blob nothing can consume.
        let Some(path) = hash.global_label() else {
            unnamed += 1;
            continue;
        };
        if !prefixes.is_empty() && !prefixes.iter().any(|p| path.starts_with(p)) {
            continue;
        }
        let size = arc
            .get_file_data_from_hash(hash, Region::UsEnglish)
            .map(|d| d.decomp_size as u64)
            .unwrap_or(0);
        kept += 1;
        kept_bytes += size;
        if !quiet {
            println!("{size}\t{path}");
        }
        if let Some(w) = writer.as_mut() {
            writeln!(w, "{path}").expect("write --out");
        }
    }

    if let Some(w) = writer.as_mut() {
        w.flush().expect("flush --out");
    }

    eprintln!();
    eprintln!("{total} paths in the arc, {unnamed} with no label in Hashes.txt");
    eprintln!(
        "{kept} matched, {:.2} GiB decompressed ({} bytes)",
        kept_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        kept_bytes
    );
    if let Some(p) = out {
        eprintln!("wrote {p}");
    }
}
