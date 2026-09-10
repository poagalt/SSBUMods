// Writes telemetry to a file on the SD card, so a playtest can be read afterwards over FTP instead
// of needing a live `cargo skyline listen` session running at the same time.
//
// WHY THIS MATTERS FOR GETTING WORK DONE. Every gameplay fact about this mod has had to come back
// through a human describing what they saw. That is the right channel for "does it look right",
// and the wrong one for "which status did it enter on frame 12" -- and the second kind is most of
// what has actually been wrong. sys-ftpd answers regardless of what the console is running, so a
// file on the card can be read mid-session without interrupting anything.
//
// THE MOUNT PREFIX IS NOT GUESSED. Homebrew writes to `sdmc:/` (nxdumptool's own strings say so)
// while Skyline plugins inside the game process usually see `sd:/`, and nothing in the vendored
// code here settles it. So the first flush TRIES each candidate and keeps whichever accepts a
// write, then says which one it was on the TCP log. A wrong guess would have meant silently no
// file, which is the worst outcome for a diagnostic.
//
// Buffered, because a file write per frame at 60fps would be its own performance bug. Events
// accumulate in memory and flush on a timer or when the buffer fills, whichever comes first.
use std::sync::Mutex;

/// Candidate mount prefixes, tried in order on the first flush.
const CANDIDATES: &[&str] = &["sd:/", "sdmc:/", ""];
/// Where the log lands once a prefix works.
const NAME: &str = "rat-telemetry.log";
/// Flush at least this often, in frames. 120 is two seconds -- frequent enough that a crash loses
/// little, rare enough that the write cost is irrelevant.
const FLUSH_FRAMES: u32 = 120;
/// Hard cap so a long session cannot grow the buffer without bound between flushes.
const MAX_LINES: usize = 2048;

struct Sink {
    /// None until a prefix has been proven to work; Some("") means every candidate failed and we
    /// have stopped trying.
    path: Option<String>,
    probed: bool,
    lines: Vec<String>,
    since_flush: u32,
    /// Bytes written this session, reported so a silently-empty log is distinguishable from a
    /// silently-failing one.
    written: u64,
}

static SINK: Mutex<Sink> = Mutex::new(Sink {
    path: None,
    probed: false,
    lines: Vec::new(),
    since_flush: 0,
    written: 0,
});

/// Queue one line. Cheap: a String push under a short lock, no I/O.
pub fn log(line: String) {
    let Ok(mut s) = SINK.lock() else { return };
    if s.lines.len() < MAX_LINES {
        s.lines.push(line);
    }
}

/// Call once per frame. Flushes on the timer or when the buffer is full.
pub fn tick() {
    let Ok(mut s) = SINK.lock() else { return };
    s.since_flush += 1;
    if s.since_flush < FLUSH_FRAMES && s.lines.len() < MAX_LINES {
        return;
    }
    s.since_flush = 0;
    flush_locked(&mut s);
}

fn flush_locked(s: &mut Sink) {
    if s.lines.is_empty() {
        return;
    }

    // Probe once. A candidate counts as working only if a real write succeeds -- checking that a
    // directory exists would not prove it is writable from inside the game process.
    if !s.probed {
        s.probed = true;
        for prefix in CANDIDATES {
            let candidate = format!("{prefix}{NAME}");
            match std::fs::write(&candidate, b"[rat] telemetry sink opened\n") {
                Ok(()) => {
                    println!("[rat] telemetry sink: writing to {candidate}");
                    s.path = Some(candidate);
                    break;
                }
                Err(e) => {
                    println!("[rat] telemetry sink: {candidate} rejected ({e})");
                }
            }
        }
        if s.path.is_none() {
            println!("[rat] telemetry sink: no writable path found, staying on the TCP log only");
        }
    }

    let Some(path) = s.path.clone() else {
        // Nowhere to put them. Drop rather than accumulate, since the TCP log already has them.
        s.lines.clear();
        return;
    };

    let body: String = s.lines.join("\n") + "\n";
    s.lines.clear();

    // Append. Opened per flush rather than held open: a handle kept across a crash or a stage
    // reload is a handle that can be left dangling, and two flushes a second do not need one.
    use std::io::Write;
    match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut f) => {
            if f.write_all(body.as_bytes()).is_ok() {
                s.written += body.len() as u64;
            }
        }
        Err(e) => println!("[rat] telemetry sink: append to {path} failed ({e})"),
    }
}
