// Pulls the whole of rom:/data.arc down the arc_network pipe as one sequential read.
//
// WHY THIS EXISTS RATHER THAN AN nxdumptool DUMP. The obvious way to get a local data.arc is
// nxdumptool's RomFS dump, and it is a dead end without `sdmc:/switch/prod.keys`: the game's RomFS
// lives inside encrypted NCA containers, so the dumper cannot read a byte without the console's
// own keys -- which means Lockpick_RCM and a payload boot.
//
// None of that is necessary. arc_network reads `rom:/data.arc` from INSIDE the running game, where
// the RomFS is already mounted and decrypted. That is why extraction has worked all along, and the
// same pipe will hand over the whole file.
//
// WHY IT IS NOT SLOW, given the per-file pull started at 2.5 files/sec. That was ~80 round trips
// per file, because smash_arc re-seeks and re-reads per file and the zstd decoder pulls in small
// chunks. A flat sequential copy has none of that: one seek, then ceil(len / BLOCK) reads, no
// decompression and no seeking. At 4 MiB blocks a 16 GiB archive is roughly 4,100 requests in
// total -- about twenty seconds of latency across the entire transfer -- so this runs at whatever
// the link will actually carry.
//
// Resumable: given an existing partial output it seeks to the end and continues, because a dropped
// connection halfway through 16 GiB should not mean starting over.
//
// usage: arc-raw <ip:port> <out.arc>
use arc_pull::block_reader::BlockReader;
use std::io::{Read, Seek, SeekFrom, Write};

/// Read size per request. Large enough that round-trip latency is irrelevant, small enough that a
/// stall shows up in the progress line instead of looking like a hang.
const BLOCK: usize = 4 * 1024 * 1024;

fn main() {
    let mut a = std::env::args().skip(1);
    let addr = a.next().unwrap_or_else(arc_pull::default_addr);
    let out_p = a.next().unwrap_or_else(|| "extracted/data.arc".into());

    let mut reader = BlockReader::new(addr.as_str()).expect("failed to connect to arc_network");

    // SeekFrom::End is the one operation that has to ask the server, and it answers with the file
    // length -- which is also the only way to know how big this is going to be.
    let len = reader.seek(SeekFrom::End(0)).expect("failed to get length");
    eprintln!(
        "rom:/data.arc is {:.2} GiB ({len} bytes)",
        len as f64 / (1024.0 * 1024.0 * 1024.0)
    );

    let already = std::fs::metadata(&out_p).map(|m| m.len()).unwrap_or(0);
    if len > 0 && already >= len {
        eprintln!("{out_p} is already complete");
        return;
    }
    if already > 0 {
        eprintln!("resuming from {:.2} GiB", already as f64 / (1024.0 * 1024.0 * 1024.0));
    }

    if let Some(parent) = std::path::Path::new(&out_p).parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&out_p)
        .expect("cannot open output");
    out.seek(SeekFrom::Start(already)).expect("seek output");
    reader.seek(SeekFrom::Start(already)).expect("seek source");

    let mut buf = vec![0u8; BLOCK];
    let mut done = already;
    let start = std::time::Instant::now();
    let mut last_report = std::time::Instant::now();

    while done < len {
        let want = ((len - done) as usize).min(BLOCK);
        // read() serves out of the block cache, so fill `want` with however many calls it takes
        // rather than assuming one read returns everything.
        let mut filled = 0usize;
        while filled < want {
            match reader.read(&mut buf[filled..want]) {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(e) => {
                    eprintln!("\nread failed at {done}: {e}");
                    eprintln!("rerun the same command to resume from here");
                    return;
                }
            }
        }
        if filled == 0 {
            eprintln!("\nsource returned no data at {done}; stopping");
            break;
        }
        out.write_all(&buf[..filled]).expect("write output");
        done += filled as u64;

        if last_report.elapsed().as_secs() >= 2 {
            last_report = std::time::Instant::now();
            let secs = start.elapsed().as_secs_f64().max(0.001);
            let rate = (done - already) as f64 / secs / (1024.0 * 1024.0);
            let remain = if rate > 0.0 {
                (len - done) as f64 / (rate * 1024.0 * 1024.0) / 60.0
            } else {
                0.0
            };
            eprint!(
                "\r  {:.2}/{:.2} GiB  {rate:.1} MiB/s  ~{remain:.0} min left   ",
                done as f64 / (1024.0 * 1024.0 * 1024.0),
                len as f64 / (1024.0 * 1024.0 * 1024.0)
            );
            let _ = std::io::stderr().flush();
        }
    }

    out.flush().expect("flush");
    eprintln!();
    let secs = start.elapsed().as_secs_f64().max(0.001);
    eprintln!(
        "wrote {out_p}: {:.2} GiB in {:.1} min, {:.1} MiB/s average, {} block requests",
        done as f64 / (1024.0 * 1024.0 * 1024.0),
        secs / 60.0,
        (done - already) as f64 / secs / (1024.0 * 1024.0),
        reader.requests
    );
    if done < len {
        eprintln!("INCOMPLETE -- rerun to resume");
    }
}
