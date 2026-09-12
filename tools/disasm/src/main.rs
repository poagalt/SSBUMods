// Disassembles a fighter's lua2cpp NRO with every call resolved to a symbol name, so the
// functions the public decomp could not name can be read.
//
// WHY THIS EXISTS. vendor/ssbu-status has every NAMED status function for the fighters this
// moveset ports from, and it has already settled Revenge's absorb, both of Game & Watch's up-B
// params and the whole shape of DK's carry. What it omits is the unnamed `FUN_*` helpers, and
// the Alolan Whip's entire catch lives in two of them: grep the whole gaogaen decomp for "catch"
// and there is not one hit, yet SpecialS/main.c clears
// FIGHTER_GAOGAEN_INSTANCE_WORK_ID_INT_BATTLE_OBJECT_ID_SWING_THROWN_FIGHTER on entry and five
// other statuses read it back. Whatever writes it is in FUN_710001ff80, which main_loop calls
// every frame.
//
// w64devkit's objdump has no aarch64 support and there is no Ghidra here, so nothing on this
// machine could read them. yaxpeax-arm is pure Rust with no C toolchain, which makes this a
// dependency rather than an installation.
//
// NAMES ARE THE WHOLE POINT. Raw mnemonics do not answer anything -- a hundred `bl $+0x117e34`
// lines are unreadable, and they are all L2CValue constructors and `app::lua_bind::*_impl`
// engine calls that the NRO's own .dynsym knows the names of. So this walks MOD0 -> .dynamic ->
// .rela.plt, matches each PLT stub to its JUMP_SLOT relocation, and prints the imported symbol
// beside every call. The interesting line then reads as itself:
//
//     bl  GrabModule__is_catch_impl
//
// ADDRESSES MAP STRAIGHT THROUGH. Ghidra loaded these NROs at 0x7100000000 and the .text segment
// starts at file offset 0, so `FUN_710001ff80` is simply file offset 0x1ff80. That is checked
// against the NRO header rather than assumed.
//
// usage:
//   disasm <nro> <ghidra-addr|+offset> [count]     disassemble, annotated
//   disasm <nro> --header
//   disasm <nro> --calls <addr> [count]            just the call targets, in order
//   disasm <nro> --sym <substring>                 find symbols by name
use std::collections::HashMap;
use yaxpeax_arch::{Decoder, U8Reader};
use yaxpeax_arm::armv8::a64::InstDecoder;

/// Where Ghidra put these modules. Every `FUN_71...` in the decomp is relative to this.
///
/// Zero in `--flat` mode: a dump of main's .text starts at the module base, so a file offset is
/// already a module-relative address and matches the symbol table exactly.
static mut GHIDRA_BASE_CELL: u64 = 0x7100_0000_00;
#[allow(non_snake_case)]
fn GHIDRA_BASE() -> u64 { unsafe { GHIDRA_BASE_CELL } }

struct Nro {
    bytes: Vec<u8>,
    text_off: u32,
    text_size: u32,
    ro_off: u32,
    ro_size: u32,
    /// PLT stub address (== file offset) -> imported symbol name.
    plt: HashMap<u64, String>,
    /// Every GOT slot with a relocation naming a symbol, imported DATA included. A module reads a
    /// global in `main` as `adrp`+`ldr` through one of these, so this map is how an
    /// `ldr x23, [xN, #off]` becomes a NAME.
    got: HashMap<u64, String>,
    /// Locally defined symbol address -> name.
    defined: HashMap<u64, String>,
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn u64_at(b: &[u8], o: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(v)
}
fn cstr_at(b: &[u8], o: usize) -> String {
    let end = b[o..].iter().position(|&c| c == 0).map(|n| o + n).unwrap_or(b.len());
    String::from_utf8_lossy(&b[o..end]).into_owned()
}

/// The part of an Itanium-mangled name worth reading. Full demangling is a project; the last
/// couple of name components are what identifies an engine call, and
/// `_ZN3app8lua_bind28GrabModule__is_catch_implEPNS_26BattleObjectModuleAccessorE` is noise
/// around `lua_bind::GrabModule__is_catch_impl`.
fn short_name(mangled: &str) -> String {
    let Some(rest) = mangled.strip_prefix("_ZN") else {
        return mangled.to_string();
    };
    let b = rest.as_bytes();
    let mut i = 0usize;
    let mut parts: Vec<String> = Vec::new();
    while i < b.len() && b[i].is_ascii_digit() {
        let mut len = 0usize;
        while i < b.len() && b[i].is_ascii_digit() {
            len = len * 10 + (b[i] - b'0') as usize;
            i += 1;
        }
        if len == 0 || i + len > b.len() {
            break;
        }
        parts.push(String::from_utf8_lossy(&b[i..i + len]).into_owned());
        i += len;
    }
    if parts.is_empty() {
        return mangled.to_string();
    }
    // Drop leading namespaces that carry no information at a glance (app, lib, lua2cpp, phx).
    let tail: Vec<&String> = parts.iter().rev().take(2).collect();
    tail.iter().rev().map(|s| s.as_str()).collect::<Vec<_>>().join("::")
}

impl Nro {
    /// A FLAT `.text` IMAGE PLUS AN EXTERNAL SYMBOL TABLE, WHICH IS HOW `main` ARRIVES.
    ///
    /// The fighter NROs come as files with their own headers, so `open` can walk MOD0 ->
    /// .dynamic -> .dynsym in place. `main` cannot: it is the title's ExeFS NSO, it is encrypted
    /// on the card, and the only copy obtainable without prod.keys is the one
    /// `src/telemetry/exedump.rs` copies out of the running process -- 60592128 bytes of raw
    /// `.text` and a separate text file of names, because the plugin walks main's .dynsym itself
    /// while it is mapped.
    ///
    /// Addresses in that symbol file are MODULE-RELATIVE, and the image starts at the module
    /// base, so a file offset IS a symbol address and the Ghidra base is zero. Every other mode
    /// in this tool then works unchanged.
    ///
    /// usage:  RAT_SYMS=vendor/main/rat-main-symbols.txt disasm --flat <text.bin> --xrefs 0xabe800
    fn open_flat(path: &str, syms: &str) -> Self {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        let mut defined = HashMap::new();
        let table = std::fs::read_to_string(syms)
            .unwrap_or_else(|e| panic!("cannot read symbol table {syms}: {e}"));
        for line in table.lines() {
            if line.starts_with('#') {
                continue;
            }
            let mut f = line.split_whitespace();
            let Some(addr) = f.next().and_then(|s| u64::from_str_radix(s, 16).ok()) else {
                continue;
            };
            let _size = f.next();
            if let Some(name) = f.next() {
                defined.insert(addr, name.to_string());
            }
        }
        eprintln!("; flat image {} bytes, {} symbols", bytes.len(), defined.len());
        let len = bytes.len() as u32;
        Nro {
            text_off: 0,
            text_size: len,
            ro_off: len,
            ro_size: 0,
            plt: HashMap::new(),
            got: HashMap::new(),
            defined,
            bytes,
        }
    }

    fn open(path: &str) -> Self {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        // NRO0 sits at 0x10, after the entry branch and the MOD0 offset.
        assert_eq!(&bytes[0x10..0x14], b"NRO0", "not an NRO: magic at 0x10 is not NRO0");
        let size = u32_at(&bytes, 0x18) as usize;
        assert_eq!(size, bytes.len(), "NRO header size {size} != file length {}", bytes.len());
        let mut nro = Nro {
            text_off: u32_at(&bytes, 0x20),
            text_size: u32_at(&bytes, 0x24),
            ro_off: u32_at(&bytes, 0x28),
            ro_size: u32_at(&bytes, 0x2c),
            plt: HashMap::new(),
            got: HashMap::new(),
            defined: HashMap::new(),
            bytes,
        };
        nro.load_symbols();
        nro
    }

    /// MOD0 -> .dynamic -> .dynsym/.dynstr/.rela.plt, then PLT stub -> imported name.
    ///
    /// An NRO is loaded flat: segment virtual addresses equal their file offsets, so every
    /// address in the dynamic section can be used as an index into `bytes` unchanged. That is
    /// only true because .text starts at 0, which `open` has already asserted.
    fn load_symbols(&mut self) {
        let b = &self.bytes;
        let mod0 = u32_at(b, 4) as usize;
        if mod0 + 8 > b.len() || &b[mod0..mod0 + 4] != b"MOD0" {
            eprintln!("warning: no MOD0 at {mod0:#x}; calls will not be named");
            return;
        }
        let dynamic = (mod0 as i64 + u32_at(b, mod0 + 4) as i32 as i64) as usize;

        let (mut strtab, mut symtab, mut jmprel, mut pltrelsz, mut syment) = (0usize, 0usize, 0usize, 0usize, 24usize);
        let (mut rela, mut relasz) = (0usize, 0usize);
        let mut p = dynamic;
        loop {
            if p + 16 > b.len() {
                break;
            }
            let tag = u64_at(b, p);
            let val = u64_at(b, p + 8) as usize;
            match tag {
                0 => break,
                2 => pltrelsz = val,
                5 => strtab = val,
                6 => symtab = val,
                7 => rela = val,
                8 => relasz = val,
                11 => syment = val,
                23 => jmprel = val,
                _ => {}
            }
            p += 16;
        }
        if strtab == 0 || symtab == 0 {
            eprintln!("warning: no dynsym; calls will not be named");
            return;
        }

        let sym_name = |idx: usize| -> String {
            let o = symtab + idx * syment;
            if o + 24 > b.len() {
                return String::new();
            }
            cstr_at(b, strtab + u32_at(b, o) as usize)
        };

        // Defined symbols, so direct calls to functions this NRO exports get named too.
        // The symbol table has no count in .dynamic, so walk until a name offset runs past
        // .rodata -- crude, but the table sits immediately before .dynstr in every one of these.
        let mut idx = 0usize;
        while symtab + (idx + 1) * syment <= strtab {
            let o = symtab + idx * syment;
            let shndx = u16_at(b, o + 6);
            let value = u64_at(b, o + 8);
            if shndx != 0 && value != 0 {
                let n = cstr_at(b, strtab + u32_at(b, o) as usize);
                if !n.is_empty() {
                    self.defined.insert(value, n);
                }
            }
            idx += 1;
        }

        // GOT slot -> imported name, from both relocation tables. R_AARCH64_JUMP_SLOT is 1026
        // and R_AARCH64_GLOB_DAT is 1025; PLT stubs only use the former, but reading both costs
        // nothing and catches calls made through a GOT-loaded pointer.
        let mut got: HashMap<u64, String> = HashMap::new();
        for (base, size) in [(jmprel, pltrelsz), (rela, relasz)] {
            let mut o = base;
            while base != 0 && o + 24 <= base + size && o + 24 <= b.len() {
                let r_offset = u64_at(b, o);
                let r_info = u64_at(b, o + 8);
                let (sym, kind) = ((r_info >> 32) as usize, r_info & 0xffff_ffff);
                if kind == 1026 || kind == 1025 {
                    let n = sym_name(sym);
                    if !n.is_empty() {
                        got.insert(r_offset, n);
                    }
                }
                o += 24;
            }
        }

        // PLT stubs. Each is the fixed four-instruction sequence
        //     adrp x16, <page>;  ldr x17, [x16, #<lo>];  add x16, x16, #<lo>;  br x17
        // so the GOT slot it reaches is recoverable from the first two words, and the stub's own
        // address is what a `bl` lands on.
        let mut a = 0usize;
        while a + 16 <= self.text_size as usize {
            let w0 = u32_at(b, a);
            let w1 = u32_at(b, a + 4);
            let w3 = u32_at(b, a + 12);
            // adrp x16 : op=1, immlo[30:29], 10000, immhi[23:5], Rd=16
            let is_adrp16 = (w0 & 0x9f00_001f) == 0x9000_0010;
            // ldr x17, [x16, #imm12*8]
            let is_ldr17 = (w1 & 0xffc0_03ff) == 0xf940_0211;
            // br x17
            let is_br17 = w3 == 0xd61f_0220;
            if is_adrp16 && is_ldr17 && is_br17 {
                let immlo = ((w0 >> 29) & 3) as u64;
                let immhi = ((w0 >> 5) & 0x7ffff) as u64;
                let imm = ((immhi << 2) | immlo) << 12;
                // sign-extend the 33-bit page offset
                let imm = ((imm as i64) << 31 >> 31) as i64;
                let page = (a as i64 & !0xfff) + imm;
                let off = (((w1 >> 10) & 0xfff) as i64) * 8;
                let slot = (page + off) as u64;
                if let Some(n) = got.get(&slot) {
                    self.plt.insert(a as u64, n.clone());
                }
            }
            a += 4;
        }
        self.got = got;
        eprintln!(
            "; {} imported PLT stubs, {} defined symbols, {} named GOT slots",
            self.plt.len(),
            self.defined.len(),
            self.got.len()
        );
    }

    /// File offset for a Ghidra address, or for a raw `+offset`.
    fn resolve(&self, spec: &str) -> usize {
        let off = if let Some(rest) = spec.strip_prefix('+') {
            usize::from_str_radix(rest.trim_start_matches("0x"), 16).expect("bad +offset")
        } else {
            let a = u64::from_str_radix(spec.trim_start_matches("0x"), 16).expect("bad address");
            if a >= GHIDRA_BASE() {
                (a - GHIDRA_BASE()) as usize
            } else {
                a as usize
            }
        };
        assert!(off < self.bytes.len(), "offset {off:#x} is past the end of the file");
        off
    }

    /// What lives at this file offset, if anything has a name for it.
    fn name_of(&self, off: u64) -> Option<String> {
        if let Some(n) = self.plt.get(&off) {
            return Some(short_name(n));
        }
        self.defined.get(&off).map(|n| short_name(n))
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.is_empty() {
        eprintln!("usage: disasm <nro> <addr> [count] | --header | --calls <addr> [count] | --sym <substr>");
        std::process::exit(2);
    }
    // `--flat` swaps the NRO reader for a raw .text image plus an external symbol table, and
    // moves the base to zero. Everything after this point is identical either way.
    let (nro, mode_idx) = if a[0] == "--flat" {
        unsafe { GHIDRA_BASE_CELL = 0 };
        let syms = std::env::var("RAT_SYMS")
            .unwrap_or_else(|_| "vendor/main/rat-main-symbols.txt".into());
        (Nro::open_flat(&a[1], &syms), 2)
    } else {
        (Nro::open(&a[0]), 1)
    };
    let a: Vec<String> = a[mode_idx..].iter().cloned().collect();
    let a: Vec<String> = std::iter::once(String::new()).chain(a).collect();
    let mode = a.get(1).map(String::as_str).unwrap_or("");

    if mode == "--header" {
        println!("size      {:#x}", nro.bytes.len());
        println!(
            "text      off {:#x} size {:#x}  (ghidra {:#x}..{:#x})",
            nro.text_off,
            nro.text_size,
            GHIDRA_BASE() + nro.text_off as u64,
            GHIDRA_BASE() + (nro.text_off + nro.text_size) as u64
        );
        println!("rodata    off {:#x} size {:#x}", nro.ro_off, nro.ro_size);
        return;
    }

    // WHO CALLS THIS, AND THEREFORE WITH WHAT ARGUMENTS.
    //
    // The reason this exists: `CaptureModule::capture(boma, u32, i32, bool, i32)` is exactly the
    // function that would let a hand-made command grab hold its victim the way the engine does --
    // inert, positioned, escapable -- and four of its five arguments are unnamed integers. Nobody
    // should guess those. The game calls it somewhere, so the game can be asked.
    //
    // `--xrefs <addr>` walks .text one word at a time and reports every `bl` or `b` whose resolved
    // target is that address. Fixed-width instructions make that exhaustive rather than heuristic.
    // WHO REFERENCES A DATA ADDRESS, which `--xrefs` cannot answer because a data reference is
    // not a branch.
    //
    // AArch64 reaches a global in two instructions -- `adrp xN, <page>` then an `add xN, xN, #lo`
    // or an `ldr`/`ldrb`/`str` with `#lo` -- so a reference is a PAIR, and neither half means
    // anything alone. This tracks the page each register was last given and reports the second
    // instruction that resolves to the address asked for.
    //
    // It is how a string in `.rodata` becomes the function that uses it. The lua_const name
    // strings live in main's .rodata; the code that turns one into the 64-bit hash the const
    // table is keyed by is whatever references it.
    //
    // Register tracking is a sliding window and deliberately naive: an `adrp` is matched against
    // the next few instructions on the same register and nothing is followed across a branch.
    // False negatives are possible, false positives are not -- every hit is a real pair.
    if mode == "--dataxref" {
        // The target is taken as a RAW ADDRESS and not resolved to a file offset: the whole point
        // is to find references to `.rodata` and `.data`, which a `--flat` image of `.text` does
        // not contain.
        let target = {
            let s = a[2].trim_start_matches("0x");
            let v = u64::from_str_radix(s, 16).expect("bad target address");
            if v >= GHIDRA_BASE() { v } else { GHIDRA_BASE() + v }
        };
        let window: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(12);
        let decoder = InstDecoder::default();
        let mut page: [Option<u64>; 32] = [None; 32];
        let mut age: [usize; 32] = [0; 32];
        let mut hits = 0;
        let mut off = 0usize;
        while off + 4 <= nro.text_size as usize {
            let pc = GHIDRA_BASE() + off as u64;
            let mut reader = U8Reader::new(&nro.bytes[off..off + 4]);
            if let Ok(inst) = decoder.decode(&mut reader) {
                let text = format!("{inst}");
                if let Some(rd) = adrp_dest(&text) {
                    page[rd] = Some((pc & !0xfff).wrapping_add(adrp_page(&text).unwrap_or(0)));
                    age[rd] = 0;
                } else {
                    for (rn, lo) in low_uses(&text) {
                        if age[rn] <= window {
                            if let Some(p) = page[rn] {
                                if p.wrapping_add(lo) == target {
                                    let owner = nro
                                        .defined
                                        .iter()
                                        .filter(|(k, _)| **k <= pc - GHIDRA_BASE())
                                        .max_by_key(|(k, _)| **k)
                                        .map(|(k, v)| {
                                            format!(
                                                "{} + {:#x}",
                                                short_name(v),
                                                (pc - GHIDRA_BASE()) - k
                                            )
                                        })
                                        .unwrap_or_else(|| "<no symbol>".into());
                                    println!("{pc:#x}  {text}   in {owner}");
                                    hits += 1;
                                }
                            }
                        }
                    }
                }
                for r in 0..32 {
                    age[r] += 1;
                }
            }
            off += 4;
        }
        eprintln!("; {hits} data reference(s)");
        return;
    }

    if mode == "--xrefs" {
        let target = GHIDRA_BASE() + nro.resolve(&a[2]) as u64;
        let decoder = InstDecoder::default();
        let mut hits = 0;
        let mut off = 0usize;
        while off + 4 <= nro.text_size as usize {
            let pc = GHIDRA_BASE() + off as u64;
            let mut reader = U8Reader::new(&nro.bytes[off..off + 4]);
            if let Ok(inst) = decoder.decode(&mut reader) {
                let text = format!("{inst}");
                if let Some((op, rest)) = text.split_once(' ') {
                    if matches!(op, "bl" | "b") {
                        if parse_branch_target(rest, pc) == Some(target) {
                            // Name the containing function if a defined symbol covers it, by
                            // taking the nearest defined symbol at or below this address.
                            let owner = nro
                                .defined
                                .iter()
                                .filter(|(k, _)| **k <= pc - GHIDRA_BASE())
                                .max_by_key(|(k, _)| **k)
                                .map(|(k, v)| format!("{} + {:#x}", short_name(v), (pc - GHIDRA_BASE()) - k))
                                .unwrap_or_else(|| "<no symbol>".into());
                            println!("{pc:#x}  {op}  in {owner}");
                            hits += 1;
                        }
                    }
                }
            }
            off += 4;
        }
        eprintln!("; {hits} call site(s)");
        return;
    }

    // EVERY STATUS FUNCTION AN NRO REGISTERS -- see `status_map`.
    if mode == "--statusmap" {
        let start = nro.resolve(a.get(2).expect("--statusmap needs the create_agent address"));
        let span: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(20000);
        status_map(&nro, start, span);
        return;
    }

    // A READABLE TRACE OF A STATUS FUNCTION -- see `is_noise` and `hash40_names` for why.
    if mode == "--trace" {
        let start = nro.resolve(a.get(2).expect("--trace needs an address"));
        let count: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(2000);
        let decoder = InstDecoder::default();
        let mut pending: Vec<String> = Vec::new();
        let mut page: [u64; 32] = [0; 32];
        let mut imm: [u64; 32] = [0; 32];
        for i in 0..count {
            let off = start + i * 4;
            if off + 4 > nro.bytes.len() {
                break;
            }
            let pc = GHIDRA_BASE() + off as u64;
            let mut reader = U8Reader::new(&nro.bytes[off..off + 4]);
            let Ok(inst) = decoder.decode(&mut reader) else { continue };
            let text = format!("{inst}");

            // A 64-bit immediate built by mov/movk, which is how every hash40 literal arrives.
            if let Some(rest) = text.strip_prefix("mov ") {
                let p: Vec<&str> = rest.split(", ").collect();
                if p.len() == 2 {
                    if let (Some(rd), Some(v)) = (reg_num(p[0]), imm_val(p[1])) {
                        imm[rd] = v;
                    }
                }
            } else if let Some(rest) = text.strip_prefix("movk ") {
                let p: Vec<&str> = rest.split(", ").collect();
                if p.len() == 3 {
                    if let (Some(rd), Some(v)) = (reg_num(p[0]), imm_val(p[1])) {
                        let sh: u32 = p[2].trim_start_matches("lsl #").parse().unwrap_or(0);
                        imm[rd] |= v << sh;
                        if let Some(n) = hash40_names().get(&imm[rd]) {
                            pending.push(format!("\"{n}\""));
                        }
                    }
                }
            } else if let Some(rd) = adrp_dest(&text) {
                page[rd] = adrp_page(&text).unwrap_or(0);
            } else if text.starts_with("ldr w") {
                if let Some(note) = lua_const_note(&text) {
                    pending.push(note.trim_start_matches("   ; ~lua_const ").to_string());
                }
            }

            if text.starts_with("bl ") {
                let target = text
                    .split_once(' ')
                    .and_then(|(_, rest)| parse_branch_target(rest, pc));
                let name = target.and_then(|t| nro.name_of(t - GHIDRA_BASE()));
                match name {
                    Some(n) if !is_noise(&n) => {
                        if pending.is_empty() {
                            println!("{pc:#x}  call {n}");
                        } else {
                            println!("{pc:#x}  call {n}   [{}]", pending.join(", "));
                        }
                        pending.clear();
                    }
                    Some(_) => {}
                    None => {
                        if let Some(t) = target {
                            println!("{pc:#x}  call {t:#x}   (unnamed)");
                            pending.clear();
                        }
                    }
                }
            } else if text.starts_with("tbz ")
                || text.starts_with("tbnz ")
                || text.starts_with("cbz ")
                || text.starts_with("cbnz ")
                || text.starts_with("b.")
            {
                println!("{pc:#x}    {text}");
            } else if text == "ret" {
                println!("{pc:#x}  --- ret ---");
                break;
            }
        }
        return;
    }

    // WHAT GLOBAL AN `adrp`+`ldr` PAIR IS READING, BY NAME.
    //
    // The lua_const array is the reason this exists. Every fighter NRO reads its constants as
    // `ldr wN, [xTABLE, #off]` where xTABLE came from `adrp xT, <page>; ldr xT, [xT, #slot]` --
    // a GOT load -- and the OFFSETS are all a disassembly can see. `CaptureModule::capture`'s
    // third argument is `lua_const[0x250]` and has been unnamed for that whole reason.
    //
    // But the GOT slot has a relocation, and the relocation names a symbol. So the pointer is
    // identifiable even though the array contents are not in any file: name it here, look the
    // name up in `main`'s own symbol table, and the array's address is known exactly -- no
    // signature scan, no dump.
    //
    // Takes the SLOT address, which is (adrp page) + (ldr offset). With no argument it lists every
    // named slot, which is how you find out what a module imports at all.
    if mode == "--got" {
        match a.get(2) {
            None => {
                let mut all: Vec<_> = nro.got.iter().collect();
                all.sort();
                for (slot, name) in all {
                    println!("{slot:#x}  {name}");
                }
                eprintln!("; {} named GOT slot(s)", nro.got.len());
            }
            Some(spec) => {
                let want = u64::from_str_radix(spec.trim_start_matches("0x"), 16)
                    .expect("bad slot address");
                match nro.got.get(&want) {
                    Some(name) => println!("{want:#x}  {name}"),
                    None => println!("{want:#x}  (no relocation names this slot)"),
                }
            }
        }
        return;
    }

    // EVERY INSTRUCTION IN .text MATCHING A SUBSTRING, WITH THE NEXT NAMED CALL AFTER IT.
    //
    // For anchoring an unnamed constant. The lua_const globals are a single array in `main`, so a
    // fighter NRO loading `[x24, #0x250]` and the common NRO loading `[x23, #0x250]` are reading
    // the SAME constant -- which means an unnamed offset can be identified by finding a use of it
    // whose consuming function IS named, in any module. `--calls` and the plain disassembly both
    // stop at the first `ret`, which makes them useless for a whole-module sweep; this does not.
    if mode == "--scan" {
        let needle = a.get(2).expect("--scan needs a substring").to_string();
        let after: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(6);
        let decoder = InstDecoder::default();
        let mut pending: Vec<(u64, String)> = Vec::new();
        let mut off = 0usize;
        while off + 4 <= nro.text_size as usize {
            let pc = GHIDRA_BASE() + off as u64;
            let mut reader = U8Reader::new(&nro.bytes[off..off + 4]);
            if let Ok(inst) = decoder.decode(&mut reader) {
                let text = format!("{inst}");
                let target = text
                    .split_once(' ')
                    .filter(|(op, _)| matches!(*op, "bl" | "b"))
                    .and_then(|(_, rest)| parse_branch_target(rest, pc));
                let name = target.and_then(|t| nro.name_of(t - GHIDRA_BASE()));

                // A hit prints itself and then waits for the next NAMED call, which is what
                // identifies the constant: the function it is handed to.
                if text.contains(&needle) {
                    pending.push((pc, text.clone()));
                }
                if let Some(n) = name {
                    pending.retain(|(hit_pc, hit_text)| {
                        if (pc - hit_pc) as usize <= after * 4 {
                            println!("{hit_pc:#x}  {hit_text}   -> {pc:#x} {n}");
                            false
                        } else {
                            true
                        }
                    });
                }
                pending.retain(|(hit_pc, _)| (pc - hit_pc) as usize <= after * 4);
            }
            off += 4;
        }
        return;
    }

    if mode == "--sym" {
        let needle = a.get(2).expect("--sym needs a substring").to_lowercase();
        let mut hits: Vec<(u64, &String, &str)> = Vec::new();
        for (k, v) in &nro.plt {
            if v.to_lowercase().contains(&needle) {
                hits.push((*k, v, "plt"));
            }
        }
        for (k, v) in &nro.defined {
            if v.to_lowercase().contains(&needle) {
                hits.push((*k, v, "def"));
            }
        }
        hits.sort();
        for (k, v, w) in hits {
            println!("{:#x}  {w}  {v}", GHIDRA_BASE() + k);
        }
        return;
    }

    let calls_only = mode == "--calls";
    let addr_arg = if calls_only { &a[2] } else { &a[1] };
    let count: usize = a
        .get(if calls_only { 3 } else { 2 })
        .and_then(|s| s.parse().ok())
        .unwrap_or(80);

    let start = nro.resolve(addr_arg);
    let decoder = InstDecoder::default();

    // DECODE ONE WORD AT A TIME. A64 is fixed width, so a fresh reader over each 4-byte slice
    // keeps the cursor exact even when a word fails to decode -- a data island in the middle of
    // a function must not desynchronise everything after it.
    for i in 0..count {
        let off = start + i * 4;
        if off + 4 > nro.bytes.len() {
            break;
        }
        let pc = GHIDRA_BASE() + off as u64;
        let word = u32_at(&nro.bytes, off);
        let mut reader = U8Reader::new(&nro.bytes[off..off + 4]);
        let (text, target) = match decoder.decode(&mut reader) {
            Ok(inst) => {
                let t = format!("{inst}");
                let tgt = t
                    .split_once(' ')
                    .filter(|(op, _)| matches!(*op, "bl" | "b"))
                    .and_then(|(_, rest)| parse_branch_target(rest, pc));
                (t, tgt)
            }
            Err(e) => (format!("<{e}>"), None),
        };
        let note = target
            .and_then(|t| nro.name_of(t - GHIDRA_BASE()).map(|n| format!("   ; {n}")))
            .or_else(|| target.map(|t| format!("   ; {t:#x}")))
            .or_else(|| lua_const_note(&text))
            .unwrap_or_default();
        if calls_only {
            if text.starts_with("bl ") {
                println!("{pc:#x} {}", note.trim_start_matches("   ; "));
            }
        } else {
            println!("{pc:#x}  {word:08x}  {text}{note}");
        }
        if text == "ret" && !calls_only {
            println!("{pc:#x}  --- ret ---");
            break;
        }
    }
}

/// Turn yaxpeax's `$+0x1234` / `$-0x1234` branch rendering into an absolute address.
fn parse_branch_target(rest: &str, pc: u64) -> Option<u64> {
    let body = rest.trim().strip_prefix('$')?;
    let (sign, hex) = if let Some(h) = body.strip_prefix('+') {
        (1i64, h)
    } else if let Some(h) = body.strip_prefix('-') {
        (-1i64, h)
    } else {
        (1i64, body)
    };
    let v = i64::from_str_radix(hex.trim_start_matches("0x"), 16).ok()?;
    Some((pc as i64 + sign * v) as u64)
}

// ---------------------------------------------------------------------------------------------
// The three text helpers `--dataxref` needs. Parsing the disassembler's own output rather than
// re-decoding the instruction fields: the printer has already normalised the shifts and scales,
// so `ldr x8, [x8, #0x920]` carries the BYTE offset and not the encoded imm12.
// ---------------------------------------------------------------------------------------------

/// `x8` -> 8, `w1` -> 1. `sp`, `xzr` and anything else -> None, which drops them from tracking
/// rather than aliasing them onto x31.
fn reg_num(r: &str) -> Option<usize> {
    let r = r.trim();
    let (x, n) = r.split_at(1);
    if x != "x" && x != "w" {
        return None;
    }
    n.parse::<usize>().ok().filter(|n| *n < 32)
}

fn reg_num_s(r: &str) -> Option<usize> {
    reg_num(r)
}

/// `#0x920` -> 0x920, `#20` -> 20.
fn imm_val(s: &str) -> Option<u64> {
    let s = s.trim().strip_prefix('#')?;
    if let Some(hex) = s.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u64>().ok()
    }
}

/// `adrp x8, $+0x4d29000` -> Some(8)
fn adrp_dest(t: &str) -> Option<usize> {
    let rest = t.strip_prefix("adrp ")?;
    reg_num(rest.split(',').next()?)
}

/// The signed page offset out of `adrp xN, $+0x...`, to be added to the PC's page.
fn adrp_page(t: &str) -> Option<u64> {
    let rest = t.strip_prefix("adrp ")?;
    let (_, imm) = rest.split_once(", ")?;
    let imm = imm.trim();
    let neg = imm.starts_with("$-");
    let hex = imm
        .trim_start_matches("$+")
        .trim_start_matches("$-")
        .trim_start_matches("0x");
    let v = u64::from_str_radix(hex, 16).ok()?;
    Some(if neg { (v as i64).wrapping_neg() as u64 } else { v })
}

/// Every `(base register, byte offset)` an instruction uses, from both `add xN, xM, #imm` and any
/// `[xM, #imm]` addressing. Register-offset forms like `[x23, x8]` carry no immediate and are
/// skipped.
fn low_uses(t: &str) -> Vec<(usize, u64)> {
    let mut out = Vec::new();
    if let Some(rest) = t.strip_prefix("add ") {
        let p: Vec<&str> = rest.split(", ").collect();
        if p.len() == 3 {
            if let (Some(rn), Some(imm)) = (reg_num(p[1]), imm_val(p[2])) {
                out.push((rn, imm));
            }
        }
    }
    if let Some(i) = t.find('[') {
        let inner = &t[i + 1..];
        if let Some(j) = inner.find(']') {
            let mut p = inner[..j].split(", ");
            if let Some(base) = p.next().and_then(reg_num_s) {
                match p.next() {
                    None => out.push((base, 0)),
                    Some(o) => {
                        if let Some(v) = imm_val(o) {
                            out.push((base, v));
                        }
                    }
                }
            }
        }
    }
    out
}

/// NAMES A lua_const READ, which is the difference between reading a status script and guessing
/// at it.
///
/// Every fighter's compiled status code reads its constants out of one i32 array --
/// `lua2cpp::L2CAgentGeneratedBase::const_value_table__`, filled at load by
/// `construct_bind_const_value_table` -- so a status kind, a work id, a flag and a link number
/// all look like `ldr wN, [x23, #0x4e4]` and carry no name at all. Six rounds of this project's
/// work stalled on exactly that: `CaptureModule::capture`'s third argument was
/// `lua_const[0x250]`, and there was no way to read it out of any file.
///
/// `vendor/ssbu-status/lua_dict.txt` is that mapping -- 19,310 `offset:NAME` lines -- and it was
/// already in the tree. Three independent cross-checks say it is the right one:
///
///   0x38  LUA_SCRIPT_STATUS_FUNC_STATUS_PRE     matches the anchor derived from sv_set_status_func
///   0xb57c FIGHTER_GAOGAEN_STATUS_KIND_FINAL_DASH  matches the FIRST constant the decompiled
///                                             create_agent_fighter_status_script_gaogaen names
///   0x4e4 LINK_NO_CAPTURE                       matches the link number derived independently
///                                             from SpecialSThrow's send_event_nodes calls
///
/// A HINT, NOT A FACT, and marked as one. Any `ldr wN, [xM, #off]` matches -- a struct field read
/// at the same offset gets a name it has nothing to do with. What makes it usable anyway is that
/// the base register is obvious in context: the const table is loaded once per function through a
/// GOT slot and then read dozens of times.
fn lua_const_note(text: &str) -> Option<String> {
    let lo = low_uses(text);
    if !text.starts_with("ldr w") || lo.len() != 1 {
        return None;
    }
    let off = lo[0].1;
    let name = lua_dict().get(&off)?;
    Some(format!("   ; ~lua_const {name}"))
}

fn lua_dict() -> &'static HashMap<u64, String> {
    static CELL: std::sync::OnceLock<HashMap<u64, String>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        let path = std::env::var("RAT_LUA_DICT")
            .unwrap_or_else(|_| "vendor/ssbu-status/lua_dict.txt".into());
        let mut m = HashMap::new();
        if let Ok(s) = std::fs::read_to_string(&path) {
            for line in s.lines() {
                if let Some((off, name)) = line.trim().split_once(':') {
                    if let Ok(v) = u64::from_str_radix(off.trim_start_matches("0x"), 16) {
                        m.insert(v, name.to_string());
                    }
                }
            }
        }
        m
    })
}

// ---------------------------------------------------------------------------------------------
// A READABLE TRACE OF A STATUS FUNCTION: every engine call in order, with the constants that were
// loaded for it.
//
// WHY THIS EXISTS. `vendor/ssbu-status` ships Ghidra-decompiled C for five fighter NROs and NOT
// for `lua2cpp_common.nro` -- and lua2cpp_common is where the VICTIM side of every grab lives.
// `SwingGaogaenCatched`, `CaptureWait`, `CapturePulled`, `MiifighterCounterThrown`: all of it has
// been read here one `--calls` invocation at a time, cross-referencing the constants by hand. That
// is how six rounds of grab bugs each took a full investigation.
//
// There is no Ghidra on this machine, so this is the substitute, and for reading a status script it
// is most of what the decompilation would give. An L2C status function is a straight sequence of
// engine calls with constants marshalled into L2CValues in between, so printing the calls in order
// with their pending constants IS the shape of the source:
//
//   call LinkModule::is_link                      [LINK_NO_CAPTURE]
//   call LinkModule::set_model_constraint_pos_ort [LINK_NO_CAPTURE, CONSTRAINT_FLAG_POSITION, ...]
//   call WorkModule::get_param_int                [param_special_s, special_s_catch_clatter_frame]
//   call ControlModule::start_clatter
//
// The L2CValue constructor and accessor traffic is dropped, which is the difference between 45
// lines of signal and 400 lines of noise. Constants accumulate across the dropped calls and are
// attributed to the next real one, because that is what the marshalling is FOR.
//
// Three sources of names, all already in the tree: the module's own symbols for the calls,
// `vendor/ssbu-status/lua_dict.txt` for `ldr wN, [xM, #off]` constant reads, and the hash40 name
// list for `mov`/`movk` immediates -- so a motion or event name appears as `"shoulder"` rather than
// as 0x8af9b3d1d.
// ---------------------------------------------------------------------------------------------

/// Calls that are marshalling rather than behaviour.
fn is_noise(name: &str) -> bool {
    name.contains("L2CValue")
        || name.contains("L2CAgent")
        || name.contains("Vector3__create")
        || name.contains("_Unwind_Resume")
}

fn hash40_names() -> &'static HashMap<u64, String> {
    static CELL: std::sync::OnceLock<HashMap<u64, String>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        let mut t = [0u32; 256];
        for i in 0..256usize {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            t[i] = c;
        }
        let mut m = HashMap::new();
        for path in [
            std::env::var("RAT_HASH_NAMES").unwrap_or_else(|_| "work/hash40-names.txt".into()),
            "vendor/Hashes.txt".to_string(),
        ] {
            if let Ok(s) = std::fs::read_to_string(&path) {
                for line in s.lines() {
                    let name = line.trim();
                    if name.is_empty() {
                        continue;
                    }
                    let mut c: u32 = 0xFFFF_FFFF;
                    for &b in name.as_bytes() {
                        c = t[((c as u8) ^ b) as usize] ^ (c >> 8);
                    }
                    let h = ((name.len() as u64) << 32) | (c ^ 0xFFFF_FFFF) as u64;
                    m.entry(h).or_insert_with(|| name.to_string());
                }
                break;
            }
        }
        m
    })
}

// ---------------------------------------------------------------------------------------------
// EVERY STATUS FUNCTION A FIGHTER NRO REGISTERS: which status, which line, and where the code is.
//
// A fighter NRO exports exactly ONE symbol for its status scripts --
// `create_agent_fighter_status_script_<name>` -- and every status function it installs is an
// internal static with no symbol at all. That is why reading Incineroar's side-B meant reading
// `vendor/ssbu-status`'s pre-decompiled tree and hoping it was complete, and why there is no such
// tree for `lua2cpp_common`.
//
// The registration recovers all of it. `create_agent` is a long run of:
//
//     mov  w8, #0xb57c ; ldr w1, [x23, x8]      the STATUS KIND, as a lua_const offset
//     ldr  w1, [x23, #0x34]                     the LINE, as a lua_const offset
//     adrp x3, <page> ; add x3, x3, #<lo>       the FUNCTION
//     bl   L2CAgentBase::sv_set_status_func
//
// so walking it yields `(status, line, address)` for every function the fighter has, with both
// names resolved through lua_dict. Feed the addresses to `tools/decomp.sh` and the whole status
// tree decompiles.
//
// This is the same recovery `vendor/ssbu-status`'s Ghidra script performed, done here so it can be
// run against any NRO -- including the ones that tree does not cover.
fn status_map(nro: &Nro, start: usize, span: usize) {
    // NO STOPPING AT THE FIRST `ret`. `create_agent` has several, because it is generated code
    // with early returns, and the registrations continue well past the first one -- which is why
    // a plain disassembly of it looks like it ends after a few hundred bytes.
    let decoder = InstDecoder::default();
    let mut page: [u64; 32] = [0; 32];
    let mut full: [u64; 32] = [0; 32];
    let mut imm: [u64; 32] = [0; 32];
    // The last two lua_const offsets seen, which are the status kind and then the line.
    let mut consts: Vec<u64> = Vec::new();
    let dict = lua_dict();
    let mut found = 0;

    let mut off = start;
    while off + 4 <= nro.text_size as usize {
        let pc = GHIDRA_BASE() + off as u64;
        let mut reader = U8Reader::new(&nro.bytes[off..off + 4]);
        let Ok(inst) = decoder.decode(&mut reader) else {
            off += 4;
            continue;
        };
        let text = format!("{inst}");

        if let Some(rd) = adrp_dest(&text) {
            page[rd] = adrp_page(&text).unwrap_or(0).wrapping_add(pc & !0xfff);
            full[rd] = 0;
        } else if let Some(rest) = text.strip_prefix("add ") {
            let p: Vec<&str> = rest.split(", ").collect();
            if p.len() == 3 {
                if let (Some(rd), Some(rn), Some(v)) = (reg_num(p[0]), reg_num(p[1]), imm_val(p[2]))
                {
                    if page[rn] != 0 {
                        full[rd] = page[rn] + v;
                    }
                }
            }
        } else if let Some(rest) = text.strip_prefix("mov ") {
            let p: Vec<&str> = rest.split(", ").collect();
            if p.len() == 2 {
                if let (Some(rd), Some(v)) = (reg_num(p[0]), imm_val(p[1])) {
                    imm[rd] = v;
                }
            }
        } else if text.starts_with("ldr w") {
            // `ldr w1, [x23, #0x34]` -- a lua_const read with an immediate, or
            // `ldr w1, [x23, x8]` -- one whose offset was built in a register first.
            if let Some(i) = text.find('[') {
                let inner = &text[i + 1..];
                if let Some(j) = inner.find(']') {
                    let mut parts = inner[..j].split(", ");
                    let _base = parts.next();
                    match parts.next() {
                        Some(o) if o.starts_with('#') => {
                            if let Some(v) = imm_val(o) {
                                consts.push(v);
                            }
                        }
                        Some(o) => {
                            if let Some(r) = reg_num(o) {
                                if imm[r] != 0 {
                                    consts.push(imm[r]);
                                }
                            }
                        }
                        None => {}
                    }
                    while consts.len() > 2 {
                        consts.remove(0);
                    }
                }
            }
        } else if text.starts_with("bl ") {
            let target = text
                .split_once(' ')
                .and_then(|(_, rest)| parse_branch_target(rest, pc));
            let name = target.and_then(|t| nro.name_of(t - GHIDRA_BASE()));
            let is_reg = name.as_deref().map_or(false, |n| n.contains("sv_set_status_func"));
            if is_reg && consts.len() == 2 {
                let unknown = "?".to_string();
                let status = dict.get(&consts[0]).unwrap_or(&unknown);
                let line = dict.get(&consts[1]).unwrap_or(&unknown);
                println!("{:#x}  {:<46} {}", full[3], status, line);
                found += 1;
                consts.clear();
            } else if is_reg {
                consts.clear();
            }
        }
        off += 4;
        if off >= start + span * 4 {
            break;
        }
    }
    eprintln!("; {found} status function(s) registered");
}
