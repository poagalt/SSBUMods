// Dumps the GAME'S OWN CODE out of the running process, to the SD card.
//
// "WHY CAN'T YOU READ THE GAME'S FILES TO FIND THE IMPLEMENTATION?" -- because the implementation
// is not in the game's files. It is worth writing down exactly where the line falls, because it
// has shaped three rounds of wrong guesses.
//
// `data.arc` -- the archive this project already extracts in full, 40,300 files for c00 -- holds
// models, skeletons, animations, `motion_list.bin`, the `.prc` params, effects and sound. It also
// holds the ACMD scripts, which ARE real game code and which this moveset copies verbatim. All of
// that has been read.
//
// What it does NOT hold is fighter STATUS logic. "If hit while the counter flag is set, absorb it
// and retaliate", "when this catch collision connects, put the victim here and me there", "while
// carrying, restrict movement to these speeds" -- none of that is scripted. It is compiled C++ in
// `main`, the NSO executable in the title's ExeFS, which is a different artifact from data.arc and
// is not extracted by any of the arc tooling here. So no amount of pulling files from the archive
// was ever going to answer these questions, and the ACMD dumps that look like source are only the
// animation layer.
//
// THE EXECUTABLE IS READABLE FROM HERE, THOUGH. A Skyline plugin is injected INTO the game process,
// so the game's `.text` is simply mapped memory in our own address space, and
// `skyline::hooks::getRegionAddress` hands over the base of each section. Copying it to the card
// needs no keys, no nxdumptool, and no console session beyond the one already happening -- which
// is the whole reason the earlier full-dump attempt stalled on `prod.keys`.
//
// AND IT HAS SYMBOLS. Every crash report this project has read resolves game functions by name --
// `cross2_Release.nss + 0x205cb40 (_ZN3app8lua_bind32MotionModule__change_motion_implE...)` -- so
// Atmosphere is reading a symbol table that is still in the binary. A dump is therefore navigable
// by name rather than by hunting through raw instructions, which is the difference between a
// tractable job and an intractable one.
//
// WHAT THIS IS AND IS NOT. It is the raw bytes of four sections plus a manifest of where each one
// started and how big it was. It is not a disassembler; reading it is a separate tool and a
// separate decision, taken once there is something to read. The runtime trace of the real fighters
// (see SOURCE_AGENTS in mod.rs) answers most behavioural questions far more cheaply, and this is
// the backstop for the ones it cannot -- exact frame counts inside a status, the precise condition
// on a branch, what a param is actually multiplied by.
//
// OFF BY DEFAULT. It writes tens of megabytes and stalls the frame it runs on, which is not
// something to do during a playtest by accident. Flip ENABLED, load into training once, and pull
// the files with tools/pull-exedump.sh.
use skyline::hooks::{getRegionAddress, Region};

use super::sink;

/// Whether to dump on the next boot. See the header: this is a deliberate one-off, not telemetry.
pub const ENABLED: bool = false;

/// Sections to write, in the order they sit in memory. `.text` is the code and the only one
/// strictly needed; `.rodata` carries the jump tables and string literals that make a
/// disassembly readable, and `.data` carries the vtables that identify which class a function
/// belongs to.
const SECTIONS: &[(&str, Region)] = &[
    ("text", Region::Text),
    ("rodata", Region::Rodata),
    ("data", Region::Data),
];

/// Guard so a per-frame caller cannot dump twice.
static mut DONE: bool = false;

pub unsafe fn run(prefix: &str) {
    if !ENABLED || DONE {
        return;
    }
    DONE = true;

    // Sizes come from the gaps between section bases, because Skyline hands over addresses and
    // not lengths. `.data`'s end is not knowable this way, so it is capped -- a short .data is
    // still useful and an unbounded read would fault.
    let text = getRegionAddress(Region::Text) as usize;
    let rodata = getRegionAddress(Region::Rodata) as usize;
    let data = getRegionAddress(Region::Data) as usize;
    let bss = getRegionAddress(Region::Bss) as usize;

    sink::diag(format!(
        "[rat] EXEDUMP bases text={text:#x} rodata={rodata:#x} data={data:#x} bss={bss:#x}"
    ));

    let bounds = [
        (text, rodata.saturating_sub(text)),
        (rodata, data.saturating_sub(rodata)),
        (data, bss.saturating_sub(data)),
    ];

    for ((name, _), (base, len)) in SECTIONS.iter().zip(bounds.iter()) {
        // A section whose computed length is absurd means the bases did not come back in the
        // order assumed. Refuse rather than read into nothing.
        if *len == 0 || *len > 0x8000_0000 {
            sink::diag(format!("[rat] EXEDUMP {name}: implausible length {len:#x}, skipped"));
            continue;
        }
        let path = format!("{prefix}rat-{name}.bin");
        let bytes = std::slice::from_raw_parts(*base as *const u8, *len);
        match std::fs::write(&path, bytes) {
            Ok(()) => sink::diag(format!("[rat] EXEDUMP wrote {path} ({len:#x} bytes from {base:#x})")),
            Err(e) => sink::diag(format!("[rat] EXEDUMP {path} failed: {e}")),
        }
    }

    // The base address is as important as the bytes. Every offset in a crash report and every
    // symbol address is relative to it, so a dump without it cannot be lined up with anything.
    let manifest = format!(
        "text {text:#x}\nrodata {rodata:#x}\ndata {data:#x}\nbss {bss:#x}\nbuild {}\n",
        super::buildstamp::BUILD
    );
    let _ = std::fs::write(format!("{prefix}rat-exedump.txt"), manifest.as_bytes());
    sink::diag("[rat] EXEDUMP complete".to_string());
}
