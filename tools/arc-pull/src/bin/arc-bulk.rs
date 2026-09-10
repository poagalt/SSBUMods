// Bulk-extracts the game's own files by walking data.arc's file table, so nothing is ever guessed.
//
// THE PROBLEM THIS ENDS. Every extraction round until now was driven by a hand-written path list,
// and one round lost nine files to invented prefixes (`h00speciallw...` for the real
// `d03speciallw...`). animof.exe patched that for animations by reading a fighter's own
// motion_list, but only for motions someone already knew to ask about. This asks the archive.
//
// HOW THE PATHS ARE RECOVERED. `FilePath::path` is the hash of the WHOLE path, and
// vendor/Hashes.txt holds path COMPONENTS -- directory paths and bare filenames -- so `path`
// resolves for almost nothing (151 of 590,711, all `prebuilt:/` fonts). But every FilePath also
// carries `parent` and `file_name` separately, and those ARE components: 583,549 of 590,711
// (98.8%) reassemble as `parent + file_name`. Measured, not assumed -- see arc-probe.rs.
//
// Crucially the DOWNLOAD still uses `FilePath::path`'s hash directly. The reassembled string is
// only ever used to filter and to name the file on disk, so a reassembly that came out wrong
// cannot fetch the wrong bytes -- it can only mis-file them, and the extension check catches that.
//
// usage:
//   arc-bulk <ip:port | path/to/data.arc> <outdir> [filters...] [--dry-run] [--skip-existing]
//
// A LOCAL data.arc IS ~100x FASTER AND THIS IS WORTH UNDERSTANDING, because the obvious
// conclusion -- "use a faster transfer tool" -- is wrong.
//
// arc_network's protocol (vendor/network_reader_lib.rs) is one byte of opcode followed by one
// seek or one read, synchronously, per request. And read_file_data calls get_file_reader() fresh
// per file, seeks (which invalidates the BufReader), then streams the zstd decode -- so a single
// 33 KB file is a seek round trip plus however many reads the decoder asks for. Measured against
// the running game: 2.5 files/sec, about 110 KB/s. That is latency, not bandwidth, so a faster
// link or a different FTP client changes nothing; the request count is the cost.
//
// The fix is to stop going through the protocol. nxdumptool can dump rom:/data.arc -- the single
// ~16 GB file arc_network is reading from -- as ONE sequential stream, after which this tool takes
// a filesystem path and every extraction is local and instant, forever, with no console involved.
// That also makes the whole archive available rather than a chosen subset.
//
//   --prefix         keep paths starting with P. Repeatable, ANY match keeps.
//   --contains S     keep only paths containing S. Repeatable, ANY match keeps -- so
//                    `--contains /c00/ --contains /param/` picks up both the costume-0 motion
//                    trees and the per-fighter param dirs, which do not sit under a costume.
//   --ext            keep only these extensions. Repeatable.
//   --dry-run        count and total bytes, download nothing. Always run this first.
//   --skip-existing  do not re-download a file already on disk at the right size.
//
// Filters default to "everything", and everything is 590k files -- run --dry-run first.
//
// `--contains /c00/` is the important one in practice: every fighter's animations exist per
// COSTUME, and all of them together are 277,141 files and 9.58 GiB. The moveset work only ever
// reads c00, because that is the slot the mod ships into.
//
// GIT BASH REWRITES ANY ARGUMENT THAT LOOKS LIKE A POSIX PATH. `--contains /c00/` arrives at the
// exe as `C:/Program Files/Git/c00/`, matches nothing, and the run reports 0 files with no error
// -- which cost two connections to diagnose. Verified with
//     h40.exe /c00/   ->  C:/Program Files/Git/c00/
// Always invoke this with MSYS2_ARG_CONV_EXCL='*', or pass the filter without a leading slash
// (`c00/`). Same class of environment trap as w64devkit's grep shadowing the real one on PATH.
//
// BE CAREFUL WITH THIS TOOL. arc_network serves rom:/data.arc from inside the RUNNING GAME, and
// opening the arc re-reads the whole compressed filesystem table over TCP each time. Three
// --dry-runs in immediate succession preceded a hard crash of the game ("the software was closed
// because an error occurred"), with the second connection failing 10053 (aborted locally) and the
// third 10061 (nothing listening). Not proven to be the cause, but the correlation is strong
// enough to work by: ONE invocation, doing all the filtering it needs, rather than several.
use arc_pull::block_reader::BlockReader;
use smash_arc::{ArcFile, ArcLookup, Hash40, Region};
use std::io::Write;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    let dry = args.iter().any(|a| a == "--dry-run");
    let skip_existing = args.iter().any(|a| a == "--skip-existing");
    args.retain(|a| a != "--dry-run" && a != "--skip-existing");

    let mut prefixes: Vec<String> = Vec::new();
    let mut exts: Vec<String> = Vec::new();
    let mut contains: Vec<String> = Vec::new();
    let mut positional: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--prefix" => {
                i += 1;
                prefixes.push(args.get(i).expect("--prefix needs a value").clone());
            }
            "--contains" => {
                i += 1;
                contains.push(args.get(i).expect("--contains needs a value").clone());
            }
            "--ext" => {
                i += 1;
                let e = args.get(i).expect("--ext needs a value").clone();
                exts.push(e.trim_start_matches('.').to_lowercase());
            }
            other => positional.push(other.to_string()),
        }
        i += 1;
    }
    let mut pos = positional.into_iter();
    let addr = pos.next().unwrap_or_else(|| "<console-ip>:43022".into());
    let outdir = pos.next().unwrap_or_else(|| "extracted".into());

    let labels = ["vendor/Hashes.txt", "../../vendor/Hashes.txt"]
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .map(|p| p.to_string())
        .expect("no Hashes.txt found -- looked in vendor/ and ../../vendor/");
    Hash40::set_global_labels_file(&labels).expect("failed to load hash labels");

    // A path to a real file means work locally and never touch the console; anything else is
    // treated as host:port. The local path is ~100x faster and is the end state -- see the header.
    let local = std::path::Path::new(&addr).is_file();
    let arc = if local {
        eprintln!("opening local {addr} ...");
        ArcFile::open(&addr).expect("failed to open local data.arc")
    } else {
        eprintln!("connecting to {addr} (block-cached client) ...");
        let reader = BlockReader::new(addr.as_str()).expect("failed to connect to arc_network");
        ArcFile::from_reader(reader).expect("failed to parse arc over network")
    };
    eprintln!(
        "arc opened. filters: {} prefix, {} contains, {} ext",
        prefixes.len(), contains.len(), exts.len()
    );

    // Pass 1: decide the work and price it. Sizes come from the file table, so this costs one
    // table read regardless of how much would be downloaded.
    let mut work: Vec<(Hash40, String, u64, u64)> = Vec::new();
    let (mut unnamed, mut total) = (0u64, 0u64);
    let mut shown = 0usize;

    for fp in arc.get_file_paths() {
        total += 1;
        let (Some(parent), Some(name)) =
            (fp.parent.hash40().global_label(), fp.file_name.hash40().global_label())
        else {
            unnamed += 1;
            continue;
        };
        // The parent label already carries its trailing slash.
        let path = format!("{}{}", parent, name);

        if !prefixes.is_empty() && !prefixes.iter().any(|p| path.starts_with(p)) {
            continue;
        }
        // A --dry-run shows the first few paths that got past the PREFIX, before the narrower
        // filters. Without this a zero-match run is indistinguishable from a wrong filter, and
        // every guess costs another connection to the running game.
        if dry && shown < 10 {
            eprintln!("  sample: {path}");
            shown += 1;
        }
        if !contains.is_empty() && !contains.iter().any(|c| path.contains(c)) {
            continue;
        }
        if !exts.is_empty() {
            let e = path.rsplit('.').next().unwrap_or("").to_lowercase();
            if !exts.contains(&e) {
                continue;
            }
        }
        let hash = fp.path.hash40();
        let size = arc
            .get_file_data_from_hash(hash, Region::UsEnglish)
            .map(|d| d.decomp_size as u64)
            .unwrap_or(0);
        // Where the bytes actually live, so the fetch can run in archive order. Same arithmetic
        // smash_arc's read_file_data uses. Sorting by this turns scattered reads into a single
        // forward sweep, which is what makes the block cache hit instead of thrash.
        let offset = arc
            .get_file_info_from_hash(hash)
            .map(|info| {
                let fd = arc.get_file_data(info, Region::UsEnglish);
                arc.get_folder_offset(info, Region::UsEnglish)
                    + arc.get_file_section_offset()
                    + ((fd.offset_in_folder as u64) << 2)
            })
            .unwrap_or(u64::MAX);
        work.push((hash, path, size, offset));
    }
    work.sort_by_key(|(_, _, _, off)| *off);

    let bytes: u64 = work.iter().map(|(_, _, s, _)| s).sum();
    eprintln!();
    eprintln!("{total} paths in the arc, {unnamed} with unresolvable components");
    eprintln!(
        "{} files match, {:.2} GiB decompressed",
        work.len(),
        bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    if dry {
        eprintln!("--dry-run: nothing downloaded");
        return;
    }

    // Pass 2: fetch. Progress goes to stderr on one rewritten line; the per-file log goes to
    // stdout so a long run can be redirected and grepped for FAIL afterwards.
    let (mut ok, mut skipped, mut failed, mut got) = (0u64, 0u64, 0u64, 0u64);
    for (n, (hash, path, size, _)) in work.iter().enumerate() {
        let dest = std::path::Path::new(&outdir).join(path);
        if skip_existing {
            if let Ok(md) = std::fs::metadata(&dest) {
                if md.len() == *size && *size != 0 {
                    skipped += 1;
                    continue;
                }
            }
        }
        match arc.get_file_contents(*hash, Region::UsEnglish) {
            Ok(data) => {
                std::fs::create_dir_all(dest.parent().unwrap()).expect("mkdir");
                std::fs::write(&dest, &data).expect("write");
                println!("OK   {:>10}  {path}", data.len());
                ok += 1;
                got += data.len() as u64;
            }
            Err(e) => {
                println!("FAIL             {path}  ({e:?})");
                failed += 1;
            }
        }
        if n % 25 == 0 {
            eprint!(
                "\r  {}/{}  ok {ok}  skip {skipped}  fail {failed}  {:.2} GiB   ",
                n + 1,
                work.len(),
                got as f64 / (1024.0 * 1024.0 * 1024.0)
            );
            let _ = std::io::stderr().flush();
        }
    }
    eprintln!();
    eprintln!(
        "done: {ok} written, {skipped} already present, {failed} failed, {:.2} GiB fetched",
        got as f64 / (1024.0 * 1024.0 * 1024.0)
    );
}
