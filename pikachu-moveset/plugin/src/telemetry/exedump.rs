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
// OFF BY DEFAULT. It writes tens of megabytes and stalls the frame it runs on. Flip ENABLED,
// load into training once, and pull with tools/pull-exedump.sh.
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
/// sys-ftpd, once. `.bss` has no knowable end from the region bases, so it takes a generous fixed
/// slice rather than being skipped -- a short .bss is still a usable prefix and an unbounded read
/// would fault.
pub const DUMP_TEXT: bool = false;
pub const DUMP_RODATA: bool = true;
pub const DUMP_DATA: bool = true;
pub const DUMP_BSS: bool = true;

/// How much `.bss` to take. The region bases give no end for it, and the constant array is
/// runtime-initialised so it may well live here.
const BSS_SLICE: usize = 48 << 20;

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
    //
    // Chunked rather than one `fs::write`: this is tens of megabytes through Skyline's fs layer,
    // and a single call that fails halfway leaves nothing. Each chunk is appended, so a partial
    // dump is still a usable prefix.
    let regions: [(&str, bool, usize, usize); 4] = [
        ("text", DUMP_TEXT, text, rodata.saturating_sub(text)),
        ("rodata", DUMP_RODATA, rodata, data.saturating_sub(rodata)),
        ("data", DUMP_DATA, data, bss.saturating_sub(data)),
        // `.bss` has no end in the region bases, so it takes a fixed slice. See BSS_SLICE.
        ("bss", DUMP_BSS, bss, BSS_SLICE),
    ];
    for (name, want, base, len) in regions {
        if !want {
            continue;
        }
        if len == 0 || len > 0x8000_0000 {
            sink::diag(format!("[rat] EXEDUMP {name} length {len:#x} is implausible, skipped"));
            continue;
        }
        let path = format!("{prefix}rat-main-{name}.bin");
        let _ = std::fs::remove_file(&path);
        use std::io::Write;
        const CHUNK: usize = 4 << 20;
        let mut written = 0usize;
        let mut ok = true;
        match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            Ok(mut f) => {
                while written < len {
                    let n = CHUNK.min(len - written);
                    let s = std::slice::from_raw_parts((base + written) as *const u8, n);
                    if f.write_all(s).is_err() {
                        ok = false;
                        break;
                    }
                    written += n;
                }
            }
            Err(e) => {
                ok = false;
                sink::diag(format!("[rat] EXEDUMP open {path} failed: {e}"));
            }
        }
        sink::diag(format!(
            "[rat] EXEDUMP {path}: wrote {written:#x} of {len:#x} bytes from {base:#x}, ok={ok}"
        ));
    }

    // The base address is as important as the bytes. Every offset in a crash report and every
    // symbol address is relative to it. BSS_SLICE is recorded too, so a reader knows the .bss
    // file is a prefix rather than the whole region.
    let manifest = format!(
        "text {text:#x}\nrodata {rodata:#x}\ndata {data:#x}\nbss {bss:#x}\nbss_slice {BSS_SLICE:#x}\nbuild {}\n",
        super::buildstamp::BUILD
    );
    let _ = std::fs::write(format!("{prefix}rat-exedump.txt"), manifest.as_bytes());
    sink::diag("[rat] EXEDUMP complete".to_string());
}
