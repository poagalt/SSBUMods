// Identifies the hash the game uses for lua_const names, then writes index -> name for the whole
// table.
//
// WHY THIS IS THE UNBLOCK. Every fighter's status code reads its constants out of ONE i32 array in
// lua2cpp_common.nro -- `lua2cpp::L2CAgentGeneratedBase::const_value_table__` -- as
// `ldr wN, [xTABLE, #off]`. A disassembly can see the OFFSET and nothing else, which is why
// `CaptureModule::capture`'s third argument has been an unnamed `lua_const[0x250]` for this whole
// project and why the whip's victim cannot be handed to the engine.
//
// `construct_bind_const_value_table` at 0x52d8c0 gives the mapping away:
//
//     x20 = const_value_table__
//     w21 = 0x4c0d                      ; 19469 constants
//     x22 = 0x6a5b70                    ; an array of 19469 eight-byte HASHES, in .rodata
//     loop { lua_bind_get_value(*x22, &v); table[i] = v; x20 += 4; x22 += 8; }
//
// So the array at 0x6a5b70 is the table's index -> name mapping, one hash per slot, IN THE FILE.
// `lib::lua_bind_get_value<int>(unsigned long, int&)` takes the hash by value, so the only thing
// between an offset and a name is knowing which 64-bit hash the game used.
//
// That is decided by measurement rather than by guessing: hash all 38,729 constant names
// skyline-smash knows under each candidate algorithm and count how many of the table's 19,469
// hashes are hit. A correct algorithm hits thousands; a wrong one hits none.
use std::collections::HashMap;
use xxhash_rust::{xxh3::xxh3_64, xxh64::xxh64};

fn fnv1a64(s: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in s {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn fnv1_64(s: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in s {
        h = h.wrapping_mul(0x100000001b3);
        h ^= b as u64;
    }
    h
}

fn crc64_table(poly: u64) -> [u64; 256] {
    let mut t = [0u64; 256];
    for i in 0..256usize {
        let mut c = (i as u64) << 56;
        for _ in 0..8 {
            c = if c & (1 << 63) != 0 { (c << 1) ^ poly } else { c << 1 };
        }
        t[i] = c;
    }
    t
}

fn crc64_be(s: &[u8], t: &[u64; 256], init: u64, xorout: u64) -> u64 {
    let mut c = init;
    for &b in s {
        c = t[(((c >> 56) as u8) ^ b) as usize] ^ (c << 8);
    }
    c ^ xorout
}

fn crc64_table_refl(poly: u64) -> [u64; 256] {
    let mut t = [0u64; 256];
    for i in 0..256usize {
        let mut c = i as u64;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ poly } else { c >> 1 };
        }
        t[i] = c;
    }
    t
}

fn crc64_le(s: &[u8], t: &[u64; 256], init: u64, xorout: u64) -> u64 {
    let mut c = init;
    for &b in s {
        c = t[((c as u8) ^ b) as usize] ^ (c >> 8);
    }
    c ^ xorout
}

/// hash40 widened: crc32 in the low 32 bits, length in the top byte. Included because it is the
/// hash this game uses everywhere else, and ruling it out by measurement beats ruling it out by
/// eyeballing the top byte.
fn hash40(s: &[u8]) -> u64 {
    let mut t = [0u32; 256];
    for i in 0..256usize {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB88320 ^ (c >> 1) } else { c >> 1 };
        }
        t[i] = c;
    }
    let mut c: u32 = 0xFFFF_FFFF;
    for &b in s {
        c = t[((c as u8) ^ b) as usize] ^ (c >> 8);
    }
    ((s.len() as u64) << 32) | (c ^ 0xFFFF_FFFF) as u64
}

mod order;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.first().map(String::as_str) == Some("--order") {
        order::run(&a[1..]);
        return;
    }
    if a.len() < 2 {
        eprintln!("usage: luaconst <lua2cpp_common.nro> <names.txt> [hash-array-addr] [count]");
        std::process::exit(2);
    }
    let bytes = std::fs::read(&a[0]).expect("cannot read the nro");
    let names: Vec<String> = std::fs::read_to_string(&a[1])
        .expect("cannot read the names")
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let addr = a
        .get(2)
        .map(|s| usize::from_str_radix(s.trim_start_matches("0x"), 16).expect("bad addr"))
        .unwrap_or(0x6a5b70);
    let count = a
        .get(3)
        .map(|s| s.parse::<usize>().expect("bad count"))
        .unwrap_or(0x4c0d);

    // In an NRO every segment's file offset equals its module offset, so an address IS an index.
    let mut hashes = Vec::with_capacity(count);
    for i in 0..count {
        let o = addr + i * 8;
        if o + 8 > bytes.len() {
            eprintln!("the hash array runs past the end of the file at index {i}");
            break;
        }
        hashes.push(u64::from_le_bytes(bytes[o..o + 8].try_into().unwrap()));
    }
    eprintln!("; {} hash(es) read from {addr:#x}", hashes.len());
    eprintln!("; {} candidate name(s)", names.len());

    let ecma_be = crc64_table(0x42F0E1EBA9EA3693);
    let ecma_le = crc64_table_refl(0xC96C5795D7870F42);
    let iso_le = crc64_table_refl(0xD800000000000000);

    let algos: Vec<(&str, Box<dyn Fn(&[u8]) -> u64>)> = vec![
        ("fnv1a64", Box::new(fnv1a64)),
        ("fnv1_64", Box::new(fnv1_64)),
        ("crc64ecma_be", Box::new(move |s| crc64_be(s, &ecma_be, 0, 0))),
        ("crc64xz", Box::new(move |s| crc64_le(s, &ecma_le, u64::MAX, u64::MAX))),
        ("crc64ecma_le", Box::new(move |s| crc64_le(s, &ecma_le, 0, 0))),
        ("crc64iso", Box::new(move |s| crc64_le(s, &iso_le, 0, 0))),
        ("hash40wide", Box::new(hash40)),
        // The xxHash family, because the game's hash function at main + 0x399f660 loads all five
        // XXH64 primes as immediates -- 0x9e3779b185ebca87, 0xc2b2ae3d27d4eb4f,
        // 0x165667b19e3779f9, 0x85ebca77c2b2ae63, 0x27d4eb2f165667c4 -- alongside two words that
        // only XXH3 uses, and dispatches on `cmp len, #0x20`. Which of the two it is, and whether
        // it hashes the name with or without its terminator, is decided here rather than by
        // reading more assembly.
        ("xxh3_64", Box::new(|s| xxh3_64(s))),
        ("xxh3_64_nul", Box::new(|s: &[u8]| {
            let mut v = s.to_vec();
            v.push(0);
            xxh3_64(&v)
        })),
        ("xxh64_seed0", Box::new(|s| xxh64(s, 0))),
        ("xxh64_nul", Box::new(|s: &[u8]| {
            let mut v = s.to_vec();
            v.push(0);
            xxh64(&v, 0)
        })),
    ];

    let want: HashMap<u64, usize> = hashes.iter().enumerate().map(|(i, &h)| (h, i)).collect();
    let mut best: Option<(&str, usize)> = None;
    for (label, f) in algos.iter() {
        let hit = names.iter().filter(|n| want.contains_key(&f(n.as_bytes()))).count();
        eprintln!("  {label:<14} {hit} of {} name(s) land in the table", names.len());
        if best.map(|(_, b)| hit > b).unwrap_or(true) {
            best = Some((label, hit));
        }
    }

    let Some((label, hit)) = best else { return };
    if hit < 100 {
        eprintln!("; NO ALGORITHM MATCHED. The hash is something else; do not guess past here.");
        std::process::exit(1);
    }
    eprintln!("; using {label} with {hit} match(es)");
    let f = &algos.iter().find(|(l, _)| *l == label).unwrap().1;
    let mut by_index: HashMap<usize, &str> = HashMap::new();
    for n in names.iter() {
        if let Some(&i) = want.get(&f(n.as_bytes())) {
            // First name wins, and a collision is reported rather than hidden.
            if let Some(prev) = by_index.insert(i, n) {
                if prev != n.as_str() {
                    eprintln!("; index {i} is claimed by both {prev} and {n}");
                }
            }
        }
    }
    let mut out: Vec<_> = by_index.into_iter().collect();
    out.sort();
    println!("# lua_const index -> name, from {label}");
    println!("# offset is index*4, which is what an `ldr wN, [xTABLE, #off]` carries");
    for (i, n) in out {
        println!("{:#x} {} {}", i * 4, i, n);
    }
}
