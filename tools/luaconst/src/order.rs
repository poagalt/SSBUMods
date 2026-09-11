// The lua_const table's index -> NAME mapping, taken from the order main registers them in.
//
// WHY ORDER AND NOT THE HASH. `construct_bind_const_value_table` fills the table by looking up
// 19,469 precomputed 64-bit hashes, one per slot, from an array in lua2cpp_common's `.rodata`.
// Naming a slot therefore means inverting that hash, and the hash is not a standard one: the
// function at main + 0x399f660 loads all five XXH64 primes and dispatches on `cmp len, #0x20`,
// but neither XXH3-64 nor XXH64 with seed 0 reproduces a single one of the 19,469 values, and the
// registrar hashes `"<namespace>:<name>"` -- it builds that string in a stack buffer with a 0x3a
// separator -- rather than the bare name. Inverting it is a transcription job with no way to
// check the answer until the whole thing is right.
//
// The order is the same information and is directly checkable. Both sides are generated from one
// master list by one build step: the NRO gets the hashes in list order, and main gets a run of
// registration calls in list order. So the Nth `bl 0x37289c0` in main's `.text` names slot N.
//
// THAT IS A HYPOTHESIS, AND IT ARRIVES WITH ITS OWN TEST. The count must come out at exactly
// 19,469, and two slots whose identity was already established by disassembling unrelated code
// must land on the right names -- `create_agent_fighter_status_script_gaogaen` reads the status
// line for a `pre` registration from offset 0x34, so slot 13 must be
// LUA_SCRIPT_STATUS_FUNC_STATUS_PRE, and it reads a gaogaen status kind from 0xb57c, so slot
// 11615 must be a FIGHTER_GAOGAEN_STATUS_KIND_*. Neither was used to build the mapping.
use std::collections::HashMap;

/// `.rodata`'s module-relative base. Constant across boots: every dump of the four region bases
/// has `rodata - text` at exactly this, and the plugin's own EXEDUMP lines agree.
const RODATA_BASE: usize = 0x39C9000;

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

/// `adrp xN, #page` -> (N, page offset), decoded from the instruction rather than from printed
/// text so this can run over 15 million instructions without formatting any of them.
fn adrp(word: u32, pc: u64) -> Option<(usize, u64)> {
    if word & 0x9F00_0000 != 0x9000_0000 {
        return None;
    }
    let rd = (word & 0x1f) as usize;
    let immlo = ((word >> 29) & 3) as u64;
    let immhi = ((word >> 5) & 0x7ffff) as u64;
    let imm = (((immhi << 2) | immlo) << 12) as i64;
    // 33-bit signed
    let imm = (imm << 31) >> 31;
    Some((rd, ((pc & !0xfff) as i64 + imm) as u64))
}

/// `add xD, xN, #imm` with no shift -> (D, N, imm)
fn add_imm(word: u32) -> Option<(usize, usize, u64)> {
    if word & 0xFFC0_0000 != 0x9100_0000 {
        return None;
    }
    let rd = (word & 0x1f) as usize;
    let rn = ((word >> 5) & 0x1f) as usize;
    let imm = ((word >> 10) & 0xfff) as u64;
    Some((rd, rn, imm))
}

/// `bl <target>` -> the target address
fn bl_target(word: u32, pc: u64) -> Option<u64> {
    if word & 0xFC00_0000 != 0x9400_0000 {
        return None;
    }
    let imm = ((word & 0x03FF_FFFF) as i64) << 38 >> 38;
    Some((pc as i64 + imm * 4) as u64)
}

fn cstr(b: &[u8], o: usize) -> String {
    let end = b[o..].iter().position(|&c| c == 0).unwrap_or(0) + o;
    String::from_utf8_lossy(&b[o..end]).into_owned()
}

pub fn run(a: &[String]) {
    let text = std::fs::read(&a[0]).expect("cannot read the text image");
    let rodata = std::fs::read(&a[1]).expect("cannot read the rodata image");
    let registrar = u64::from_str_radix(a[2].trim_start_matches("0x"), 16).expect("bad registrar");

    // x1 is the name string, and it is always built as adrp+add immediately before the call. Only
    // those two registers matter, so tracking is a flat array with no aliasing analysis.
    let mut page = [0u64; 32];
    let mut full = [0u64; 32];
    let mut names: Vec<(u64, String)> = Vec::new();
    let mut missing = 0usize;

    let mut o = 0usize;
    while o + 4 <= text.len() {
        let pc = o as u64;
        let w = u32_at(&text, o);
        if let Some((rd, p)) = adrp(w, pc) {
            page[rd] = p;
            full[rd] = 0;
        } else if let Some((rd, rn, imm)) = add_imm(w) {
            if page[rn] != 0 && rd == rn {
                full[rd] = page[rn] + imm;
            } else if page[rn] != 0 {
                full[rd] = page[rn] + imm;
            }
        } else if bl_target(w, pc) == Some(registrar) {
            let addr = full[1] as usize;
            if addr >= RODATA_BASE && addr - RODATA_BASE < rodata.len() {
                names.push((pc, cstr(&rodata, addr - RODATA_BASE)));
            } else {
                missing += 1;
                names.push((pc, String::new()));
            }
        }
        o += 4;
    }

    eprintln!("; {} registration call(s), {missing} with no resolvable name", names.len());
    let named: HashMap<usize, &str> = names
        .iter()
        .enumerate()
        .map(|(i, (_, n))| (i, n.as_str()))
        .collect();
    for (slot, want) in [
        (13usize, "LUA_SCRIPT_STATUS_FUNC_STATUS_PRE"),
        (21, "LUA_SCRIPT_STATUS_FUNC_STATUS_END"),
    ] {
        match named.get(&slot) {
            Some(n) => eprintln!("; slot {slot} (offset {:#x}) = {n}   [expected {want}]", slot * 4),
            None => eprintln!("; slot {slot} is past the end"),
        }
    }
    if let Some(n) = named.get(&11615) {
        eprintln!("; slot 11615 (offset 0xb57c) = {n}   [expected a FIGHTER_GAOGAEN_STATUS_KIND_*]");
    }

    println!("# lua_const slot -> name, in main's registration order");
    println!("# offset  slot  name        (offset is what an `ldr wN, [xTABLE, #off]` carries)");
    for (i, (pc, n)) in names.iter().enumerate() {
        println!("{:#x} {} {} {:#x}", i * 4, i, n, pc);
    }
}
