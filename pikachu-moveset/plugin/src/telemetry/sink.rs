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
/// Flush at least this often, in frames.
///
/// IT WAS 120, AND A CRASH THEREFORE LOST THE MOVE THAT CAUSED IT. The session that crashed on
/// the Alolan Whip's rope throw left a 365-byte log: the install lines and nothing else. Two
/// seconds of buffer is two seconds of exactly the frames worth reading, and the crash reports
/// had to carry the whole diagnosis alone.
///
/// Eight frames is an eighth of a second. A few hundred bytes appended eight times a second is
/// nothing next to what the game is already doing, and it bounds what a crash can destroy to
/// something smaller than a single move.
const FLUSH_FRAMES: u32 = 8;
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

/// Queue one line AND write it out immediately.
///
/// FOR DIAGNOSING A CRASH, which the buffered path cannot do. Anything that kills the game before
/// the next 120-frame flush is lost, and the crashes worth diagnosing here all happen during
/// fighter load -- before a single frame of telemetry ever flushes. The TCP log is no help either,
/// because it needs a listener attached at the moment it happens.
///
/// So this pays a file open per line, deliberately. It is for install-time and load-time markers,
/// where there are a handful of lines total and the cost is irrelevant, and it means the file on
/// the card tells you how far the plugin got before it died.
pub fn diag(line: String) {
    println!("{line}");
    let Ok(mut s) = SINK.lock() else { return };
    s.lines.push(line);
    flush_locked(&mut s);
}

/// A breadcrumb: written to the card BEFORE a call that might not return.
///
/// WHY THIS IS NOT JUST ANOTHER LOG LINE. Three of the crashes this project has diagnosed were
/// the same shape -- we handed a victim, or ourselves, to engine code written for a different
/// fighter, and it dereferenced something that fighter has and Pikachu does not. The Alolan
/// Whip's rope throw is the clearest: `SWING_GAOGAEN_THROWN` calls `LinkModule::get_parent_pos`
/// for Incineroar's ROPE article, unguarded, and there is no rope, so it reads through null.
///
/// Every one of those was found from the Atmosphere crash report, which names the engine
/// function but not WHICH OF OUR CALLS reached it or with what arguments. A crumb written
/// immediately before the call closes that gap: if the game dies, the last line in the log is
/// the thing we were about to do and to whom.
///
/// Deliberately unbuffered and deliberately not on the TCP log -- the point is the bytes being
/// on the card before control leaves our code.
pub fn crumb(line: String) {
    let Ok(mut s) = SINK.lock() else { return };
    s.lines.push(line);
    flush_locked(&mut s);
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
            // THE PROBE USED TO BE `fs::write`, WHICH TRUNCATES -- SO EVERY LAUNCH ERASED THE
            // PREVIOUS SESSION'S LOG.
            //
            // That destroyed the only record of the run that mattered. A crash closes the game;
            // the next launch wipes the file; by the time anyone reads it, it contains a clean
            // session with no crash in it. Round 2 lost the cargo freeze exactly that way -- the
            // log showed no `throw_f` at all and zero CARGO lines, not because the code did not
            // run but because its evidence had already been overwritten.
            //
            // So the previous log is ROTATED to `.prev` first, and this one is appended to. Two
            // sessions survive at once, which is what "it crashed, then I restarted and played
            // more" actually needs.
            // THE DESTINATION IS REMOVED FIRST, AND THE RESULT IS REPORTED. Horizon's filesystem
            // fails a rename onto an existing path rather than replacing it, and this discarded
            // the error -- so `.prev` was created by the FIRST run that ever wrote a log and then
            // froze there forever, while every later session was truncated by the `write` below
            // at the next launch. The intent described above was never once achieved.
            //
            // Found 2026-09-18 when the log from the Yato playtest was wanted and `.prev` turned
            // out to hold build 2e8065d from 2026-09-10, eight days and many sessions earlier.
            // That is the exact failure the comment above says this code exists to prevent.
            let prev = format!("{prefix}{NAME}.prev");
            let _ = std::fs::remove_file(&prev);
            let rotated = std::fs::rename(&candidate, &prev).is_ok();
            match std::fs::write(&candidate, b"[rat] telemetry sink opened\n") {
                Ok(()) => {
                    // Says whether the rotation ACTUALLY happened, because silently believing it
                    // had is what lost eight days of logs.
                    if rotated {
                        println!("[rat] telemetry sink: writing to {candidate}, previous run kept as {prev}");
                    } else {
                        println!("[rat] telemetry sink: writing to {candidate}, no previous run to rotate");
                    }
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

/// The mount prefix the sink proved writable, for anything else that wants to write to the card.
///
/// EXISTS SO THE PREFIX IS PROBED ONCE. `sd:/` versus `sdmc:/` is settled by trying a real write
/// and keeping whichever worked (see `flush_locked`), and that answer should not be re-derived --
/// or worse, guessed differently -- by a second writer. Returns an empty string before the first
/// flush, which callers should treat as "not ready yet" rather than as the current directory.
pub fn prefix() -> String {
    let Ok(s) = SINK.lock() else { return String::new() };
    match &s.path {
        Some(p) => p.trim_end_matches(NAME).to_string(),
        None => String::new(),
    }
}
