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
const GHIDRA_BASE: u64 = 0x7100_0000_00;

struct Nro {
    bytes: Vec<u8>,
    text_off: u32,
    text_size: u32,
    ro_off: u32,
    ro_size: u32,
    /// PLT stub address (== file offset) -> imported symbol name.
    plt: HashMap<u64, String>,
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
        eprintln!(
            "; {} imported PLT stubs, {} defined symbols",
            self.plt.len(),
            self.defined.len()
        );
    }

    /// File offset for a Ghidra address, or for a raw `+offset`.
    fn resolve(&self, spec: &str) -> usize {
        let off = if let Some(rest) = spec.strip_prefix('+') {
            usize::from_str_radix(rest.trim_start_matches("0x"), 16).expect("bad +offset")
        } else {
            let a = u64::from_str_radix(spec.trim_start_matches("0x"), 16).expect("bad address");
            if a >= GHIDRA_BASE {
                (a - GHIDRA_BASE) as usize
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
    let nro = Nro::open(&a[0]);
    let mode = a.get(1).map(String::as_str).unwrap_or("");

    if mode == "--header" {
        println!("size      {:#x}", nro.bytes.len());
        println!(
            "text      off {:#x} size {:#x}  (ghidra {:#x}..{:#x})",
            nro.text_off,
            nro.text_size,
            GHIDRA_BASE + nro.text_off as u64,
            GHIDRA_BASE + (nro.text_off + nro.text_size) as u64
        );
        println!("rodata    off {:#x} size {:#x}", nro.ro_off, nro.ro_size);
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
            println!("{:#x}  {w}  {v}", GHIDRA_BASE + k);
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
        let pc = GHIDRA_BASE + off as u64;
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
            .and_then(|t| nro.name_of(t - GHIDRA_BASE).map(|n| format!("   ; {n}")))
            .or_else(|| target.map(|t| format!("   ; {t:#x}")))
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
