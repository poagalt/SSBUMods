// Dumps the GAME'S OWN CODE AND SYMBOL TABLE out of the running process, to the SD card.
//
// "WHY CAN'T YOU READ THE GAME'S FILES TO FIND THE IMPLEMENTATION?" -- because the implementation
// is not in the game's files. It is worth writing down exactly where the line falls, because it
// has shaped six rounds of hand-written status logic.
//
// `data.arc` -- the archive this project extracts in full -- holds models, skeletons, animations,
// `motion_list.bin`, the `.prc` params, effects, sound, and the ACMD scripts. All of it is read
// and all of it is copied verbatim. `prebuilt:/nro/release/lua2cpp_*.nro`, also inside data.arc,
// holds each fighter's STATUS SCRIPTS, and those are read too.
//
// What neither holds is the part that keeps needing to be reimplemented by hand. "When this catch
// collision connects, put the victim in that status and me in this one." "If hit while the counter
// flag is set, absorb it and retaliate." "While carrying, hold the victim here." `disasm --sym
// catch` over the whole gaogaen NRO finds no `CatchModule` import at all, and `grep -ri catch`
// over its entire decompiled status tree returns nothing -- because that logic is compiled C++ in
// `main`, the NSO in the title's ExeFS, and it keys on the fighter kind. It is the reason every
// one of these moves is a reimplementation rather than a drag-and-drop.
//
// THE EXECUTABLE IS READABLE FROM HERE. A Skyline plugin is injected INTO the game process, so
// `main`'s `.text` is mapped memory in our own address space and `getRegionAddress` hands over
// each section's base. No keys, no nxdumptool, no prod.keys -- which is what stalled the earlier
// attempt at this.
//
// AND IT HAS A SYMBOL TABLE, WHICH IS THE WHOLE GAME. Every crash report this project has read
// resolves game functions by name -- `cross2_Release.nss + 0x205cb40
// (_ZN3app8lua_bind32MotionModule__change_motion_implE...)` -- so the names are still in the
// binary, reachable the same way `tools/disasm` reaches them in an NRO: MOD0 -> .dynamic ->
// .dynsym. Dumping that first, as text, costs a few megabytes and turns 58 MB of ARM into
// something navigable. `app::FighterGaogaen*`, if those symbols exist, is the answer to the whip;
// the same for DK's carry and Incineroar's counter.
//
// ORDER MATTERS: symbols are written BEFORE the code, because they are small, they are the part
// that makes the rest usable, and a failed 58 MB write should not cost them.
//
// OFF BY DEFAULT. Flip ENABLED, load into training once, stay in the match half a minute, and
// pull with tools/pull-exedump.sh. The bytes go out from a background thread that starts ten
// seconds in -- see `write_regions` for the crash that bought that design.
use skyline::hooks::{getRegionAddress, Region};

use super::sink;

/// Whether to dump on the next fighter load. A deliberate one-off, not telemetry.
pub const ENABLED: bool = true;

/// WHICH REGIONS TO WRITE, AND WHY ALL OF THEM RATHER THAN JUST `.text`.
///
/// The first pass took `.text` and the symbol table only, on the reasoning that code and names
/// are what you read. That was too thin, and both of the questions it could not answer are
/// answered by the regions it skipped:
///
/// `.rodata` and `.data` hold the VTABLES. 4917 exported names is a tiny fraction of ~39,600
/// functions, and `FighterGaogaen`, `FighterDonkey` and `FighterGamewatch` are not among them --
/// internal statics are not exported. A vtable is how an unexported class is identified anyway:
/// find the table, and every slot in it is one of that class's methods at a known index. The
/// decompiled `CaptureModule.h` in the public 13.0.4 decomp is built exactly that way, indices
/// and all.
///
/// `.data` or `.bss` holds the LUA_CONST ARRAY. Every fighter NRO reads its constants out of one
/// global i32 array in `main` -- `ldr w1, [x24, #0x250]` and so on -- and the offsets are all
/// this project can see from an NRO, which is why `CaptureModule::capture`'s third argument is
/// still unnamed. With the array dumped the offsets become VALUES, and smash-rs already has
/// ~19,000 name-to-value pairs to match them against. The array is findable by signature: the
/// status-line conditions sit at offsets 0x38, 0x3c and 0x50 and must read 0, 1 and 2
/// (LUA_SCRIPT_STATUS_FUNC_STATUS_PRE / _STATUS / _STATUS_END), with 0xC (CHECK_ATTACK) at
/// 0x3074.
///
/// So: everything, and deliberately more than is needed. About 120 MB total, five minutes over
/// sys-ftpd, once. `.bss` gets its length from MOD0 -- see `bss_len`, and the crash that taught
/// it that a fixed guess is not acceptable.
///
/// `.text` and `.rodata` are already pulled and versioned under `vendor/main/` (60,592,128 and
/// 22,339,584 bytes), and neither changes between builds of THIS plugin -- only between game
/// patches. Re-dumping them would cost two minutes of background writing for bytes already on
/// disk, so they stay off until the game updates.
pub const DUMP_TEXT: bool = false;
pub const DUMP_RODATA: bool = false;
pub const DUMP_DATA: bool = true;
pub const DUMP_BSS: bool = true;

/// Ceiling on the `.bss` read, as a sanity bound rather than a guess. See `bss_len`.
const BSS_MAX: usize = 64 << 20;

/// The REAL length of `.bss`, out of the MOD0 header.
///
/// THE PREVIOUS VERSION GUESSED 48 MB AND CRASHED THE GAME. `getRegionAddress` hands over four
/// bases and no lengths, so `.text`, `.rodata` and `.data` get their sizes from the gaps to the
/// next base -- correct, and those three dumped cleanly. `.bss` is last, has no next base, and
/// took a fixed 48 MB slice on the reasoning that a short prefix is still useful. Reading 48 MB
/// from a region that is smaller than that reads unmapped memory, which is not a short prefix,
/// it is a Data Abort. Playtest: "I tried loading into training mode with incin and the game
/// crashed."
///
/// MOD0 has the answer and this function was already parsing MOD0 for `.dynamic`. Its layout:
///
///     0x00  "MOD0"
///     0x04  s32  offset to .dynamic
///     0x08  s32  offset to bss start
///     0x0c  s32  offset to bss end
///     0x10  s32  offset to eh_frame_hdr start
///
/// every offset relative to the MOD0 address itself. So the length is exact.
///
/// It is also CROSS-CHECKED rather than trusted: the computed bss start must agree with the base
/// `getRegionAddress` reported, and the length must be under `BSS_MAX`. Either check failing skips
/// `.bss` entirely, because a wrong length here costs a crash and a whole test round, and `.bss`
/// is the least important of the four.
unsafe fn bss_len(mod0: usize, bss_base: usize) -> usize {
    let start = (mod0 as i64 + u32_at(mod0, 8) as i32 as i64) as usize;
    let end = (mod0 as i64 + u32_at(mod0, 0xc) as i32 as i64) as usize;
    if end <= start {
        sink::diag(format!("[rat] EXEDUMP bss: MOD0 gives start {start:#x} end {end:#x}, skipping"));
        return 0;
    }
    let len = end - start;
    if start != bss_base {
        sink::diag(format!(
            "[rat] EXEDUMP bss: MOD0 start {start:#x} disagrees with the region base \
             {bss_base:#x}; skipping rather than reading somewhere unmapped"
        ));
        return 0;
    }
    if len > BSS_MAX {
        sink::diag(format!("[rat] EXEDUMP bss: length {len:#x} over the {BSS_MAX:#x} ceiling, skipping"));
        return 0;
    }
    sink::diag(format!("[rat] EXEDUMP bss: MOD0 says {len:#x} bytes at {start:#x}"));
    len
}

/// Guard so a per-frame caller cannot dump twice.
static mut DONE: bool = false;

fn u32_at(base: usize, off: usize) -> u32 {
    unsafe { std::ptr::read_unaligned((base + off) as *const u32) }
}
fn u64_at(base: usize, off: usize) -> u64 {
    unsafe { std::ptr::read_unaligned((base + off) as *const u64) }
}
fn u16_at(base: usize, off: usize) -> u16 {
    unsafe { std::ptr::read_unaligned((base + off) as *const u16) }
}

/// Read a NUL-terminated string out of the process, with a sanity bound.
unsafe fn cstr(addr: usize) -> String {
    let mut v = Vec::with_capacity(64);
    for i in 0..512usize {
        let c = *((addr + i) as *const u8);
        if c == 0 {
            break;
        }
        v.push(c);
    }
    String::from_utf8_lossy(&v).into_owned()
}

pub unsafe fn run(prefix: &str) {
    if !ENABLED || DONE || prefix.is_empty() {
        return;
    }
    DONE = true;

    let text = getRegionAddress(Region::Text) as usize;
    let rodata = getRegionAddress(Region::Rodata) as usize;
    let data = getRegionAddress(Region::Data) as usize;
    let bss = getRegionAddress(Region::Bss) as usize;

    sink::diag(format!(
        "[rat] EXEDUMP bases text={text:#x} rodata={rodata:#x} data={data:#x} bss={bss:#x}"
    ));

    // ---- the symbol table ------------------------------------------------------------------
    //
    // Same walk `tools/disasm` does on an NRO, because an NSO's dynamic section has the same
    // shape: the module's MOD0 offset sits at base+4, MOD0 carries a signed offset to .dynamic,
    // and every d_ptr in there is relative to the module base.
    let mod0 = text + u32_at(text, 4) as usize;
    let magic = u32_at(mod0, 0);
    let have_mod0 = &magic.to_le_bytes() == b"MOD0";
    if &magic.to_le_bytes() != b"MOD0" {
        sink::diag(format!(
            "[rat] EXEDUMP no MOD0 at {mod0:#x} (read {magic:#x}); cannot locate .dynamic"
        ));
    } else {
        let dynamic = (mod0 as i64 + u32_at(mod0, 4) as i32 as i64) as usize;
        let (mut strtab, mut symtab, mut syment) = (0usize, 0usize, 24usize);
        let mut p = dynamic;
        for _ in 0..256 {
            let tag = u64_at(p, 0);
            let val = u64_at(p, 8) as usize;
            match tag {
                0 => break,
                5 => strtab = text + val,
                6 => symtab = text + val,
                11 => syment = val,
                _ => {}
            }
            p += 16;
        }
        sink::diag(format!(
            "[rat] EXEDUMP dynamic={dynamic:#x} symtab={symtab:#x} strtab={strtab:#x} syment={syment}"
        ));

        if symtab != 0 && strtab > symtab {
            // The table has no declared count, so walk until it would run into .dynstr -- the
            // same bound that worked for all six NROs.
            let mut out = String::with_capacity(8 << 20);
            out.push_str(&format!("# main symbol dump; text base {text:#x}\n"));
            out.push_str("# columns: module-relative addr, size, name\n");
            let mut n = 0usize;
            let mut idx = 0usize;
            while symtab + (idx + 1) * syment <= strtab {
                let e = symtab + idx * syment;
                let name_off = u32_at(e, 0) as usize;
                let shndx = u16_at(e, 6);
                let value = u64_at(e, 8) as usize;
                let size = u64_at(e, 16);
                idx += 1;
                if shndx == 0 || value == 0 || name_off == 0 {
                    continue;
                }
                let nm = cstr(strtab + name_off);
                if nm.is_empty() {
                    continue;
                }
                out.push_str(&format!("{:x} {:x} {}\n", value, size, nm));
                n += 1;
                // A cap, because an implausible strtab would otherwise walk off into nothing and
                // a 200 MB string is its own failure.
                if n > 400_000 {
                    out.push_str("# TRUNCATED at 400000 symbols\n");
                    break;
                }
            }
            let path = format!("{prefix}rat-main-symbols.txt");
            match std::fs::write(&path, out.as_bytes()) {
                Ok(()) => sink::diag(format!("[rat] EXEDUMP wrote {path}, {n} symbols")),
                Err(e) => sink::diag(format!("[rat] EXEDUMP {path} failed: {e}")),
            }
        }
    }

    // ---- the regions -----------------------------------------------------------------------
    // A `vec!` rather than an array: this crate is edition 2018, where `[T; N].into_iter()` is
    // still `iter()` and hands out references.
    let plan: Vec<(&'static str, usize, usize)> = vec![
        ("text", DUMP_TEXT, text, rodata.saturating_sub(text)),
        ("rodata", DUMP_RODATA, rodata, data.saturating_sub(rodata)),
        ("data", DUMP_DATA, data, bss.saturating_sub(data)),
        // Exact, from MOD0, and skipped entirely if that cannot be confirmed. The 48 MB guess
        // this replaces read unmapped memory, which is a fault and not a short prefix.
        ("bss", DUMP_BSS, bss, if have_mod0 { bss_len(mod0, bss) } else { 0 }),
    ]
    .into_iter()
    .filter(|&(name, want, _, len)| {
        if !want {
            return false;
        }
        if len == 0 || len > 0x8000_0000 {
            sink::diag(format!("[rat] EXEDUMP {name} length {len:#x} is implausible, skipped"));
            return false;
        }
        true
    })
    .map(|(name, _, base, len)| (name, base, len))
    .collect();

    // The base addresses are as important as the bytes -- every offset in a crash report and
    // every symbol address is relative to one -- and they are handed to the writer rather than
    // written here, so that the manifest appearing on the card MEANS the regions finished.
    let manifest = format!(
        "text {text:#x}\nrodata {rodata:#x}\ndata {data:#x}\nbss {bss:#x}\nbuild {}\n",
        super::buildstamp::BUILD
    );
    sink::diag(format!(
        "[rat] EXEDUMP handed {} region(s) to the background writer; results land in \
         rat-exedump.txt, not here",
        plan.len()
    ));
    if !plan.is_empty() {
        let owned = prefix.to_string();
        std::thread::spawn(move || write_regions(owned, plan, manifest));
    }
}

/// Writes the regions to the card FROM ITS OWN THREAD, slowly and on purpose.
///
/// THIS COST A CRASH AND A TEST ROUND, so the reasoning is worth keeping. The first version ran
/// the region loop inline in `run`, which is called on the FIRST FRAME OF THE PIKACHU OBJECT --
/// i.e. during the load, while the game's other threads are still streaming resources off the
/// same SD card. 22 MB of synchronous `write_all` there produced:
///
///   * a Data Abort on `nus3::audio update` with a fault address of 0 -- the audio streamer got a
///     null buffer back, because its read never landed;
///   * `SELFTEST motions 59/77` in the same run, down from 77/77 -- animation resolution fails
///     the same way for the same reason;
///   * a log that stops dead after the `.rodata` line, one region in.
///
/// It was never the dump's own memory reads that were wrong. It was holding the only I/O device
/// the loader has, for seconds, at the one moment it is busiest.
///
/// So: a separate thread (`vendor/arc_network_lib.rs` proves `std::thread::spawn` works in a
/// Skyline plugin), a ten-second head start so the match is fully loaded before the first byte,
/// 1 MB chunks, and a rest between each one so the SD queue drains and the audio thread gets its
/// turn. Total cost about twenty seconds of wall clock, none of it on the game thread.
///
/// It also does NOT call `sink::diag`. The log buffer is `static mut` owned by the game thread,
/// and racing it from here would trade a starvation crash for a corruption crash. Everything this
/// function has to say goes into the manifest file, which only it writes.
fn write_regions(prefix: String, plan: Vec<(&'static str, usize, usize)>, manifest: String) {
    const CHUNK: usize = 1 << 20;
    const HEAD_START_MS: u64 = 10_000;
    const REST_MS: u64 = 40;

    std::thread::sleep(std::time::Duration::from_millis(HEAD_START_MS));

    let mut report = manifest;
    for (name, base, len) in plan {
        let path = format!("{prefix}rat-main-{name}.bin");
        let _ = std::fs::remove_file(&path);
        use std::io::Write;
        let mut written = 0usize;
        let mut err = String::new();
        match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            Ok(mut f) => {
                while written < len {
                    let n = CHUNK.min(len - written);
                    // Reading the game's own mapped image. The bases come from
                    // `getRegionAddress` and the lengths from the next base up or MOD0, so this
                    // stays inside the module.
                    let s = unsafe { std::slice::from_raw_parts((base + written) as *const u8, n) };
                    if let Err(e) = f.write_all(s) {
                        err = format!(" write failed: {e}");
                        break;
                    }
                    written += n;
                    std::thread::sleep(std::time::Duration::from_millis(REST_MS));
                }
            }
            Err(e) => err = format!(" open failed: {e}"),
        }
        report.push_str(&format!(
            "{name} {base:#x} wrote {written:#x} of {len:#x}{err}\n"
        ));
    }
    let _ = std::fs::write(format!("{prefix}rat-exedump.txt"), report.as_bytes());
}
