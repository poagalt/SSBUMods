// Shared across the arc-* binaries. See block_reader.rs for why the stock network client is not
// good enough on its own.
pub mod block_reader;

/// The console to read from when no address is given on the command line.
///
/// Same rule as `tools/console.sh`: FTP_HOST or SWITCH_HOST, then `.console-host.local` (looked
/// for in the current directory and each parent, so it is found from the repo root or from
/// anywhere under it), and otherwise STOP. These binaries used to fall back to <console-ip>, the
/// development console, which since the second console became the one being played means a
/// silent read from the wrong game version. See that file for the history.
pub fn default_addr() -> String {
    let host = std::env::var("FTP_HOST")
        .ok()
        .filter(|h| !h.is_empty())
        .or_else(|| std::env::var("SWITCH_HOST").ok().filter(|h| !h.is_empty()))
        .or_else(|| {
            let mut dir = std::env::current_dir().ok()?;
            loop {
                if let Ok(s) = std::fs::read_to_string(dir.join(".console-host.local")) {
                    let s = s.trim().to_string();
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
                if !dir.pop() {
                    return None;
                }
            }
        });
    match host {
        Some(h) => format!("{h}:43022"),
        None => {
            eprintln!("  no console set: write its address to .console-host.local, or pass one");
            std::process::exit(2);
        }
    }
}
